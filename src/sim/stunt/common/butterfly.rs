//! Бабочка: влетает от края волнами, персонаж бежит за ней, подпрыгивает и щёлкает
//! клювом, а она увёртывается. Потом улетает за край, и он смотрит ей вслед.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, скорости — px/с, время — с.
const TIMEOUT: f32 = 14.0;
const FLY_SECS: f32 = 7.0;
const SPEED: f32 = 80.0;
const LEAVE_SPEED: f32 = 150.0;
/// Волна полёта: поперечный размах скорости и частота.
const WAVE: f32 = 0.9;
const WAVE_HZ: f32 = 0.5;
/// Высота полёта: середина, размах и частота.
const HEIGHT: [f32; 2] = [45.0, 15.0];
const HEIGHT_HZ: f32 = 0.35;
const SNAP_REACH: f32 = 40.0;
const SNAP_SECS: f32 = 0.45;
/// Передышка между щелчками.
const SNAP_GAP: f32 = 1.1;
/// Рывок бабочки от щелчка, затухает за секунду.
const DODGE: f32 = 220.0;
const DODGE_DECAY: f32 = 3.5;
const WATCH_SECS: f32 = 1.4;

#[derive(Debug)]
enum Phase {
    Fly(f32),
    Leave,
    Watch(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Butterfly {
    fly: PropId,
    /// Ведёт бабочку сама: точка на земле под ней.
    at: Vec2,
    way: Vec2,
    dodge: Vec2,
    /// Время полёта: волны.
    t: f32,
    snap: f32,
    cooldown: f32,
    phase: Phase,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_butterfly(&mut self) -> Option<Butterfly> {
        let area = self.bounds.nearest(self.body.pos);
        let from_left = self.body.pos.x > area.center().x;
        let x = if from_left { area.min.x } else { area.max.x };
        let y = self.rng.range(area.min.y, area.max.y);
        let at = area.clamp(Vec2::new(x, y), 0.0);
        let way = Vec2::new(if from_left { 1.0 } else { -1.0 }, 0.0);
        let z = HEIGHT[0] * self.ui;
        let fly = self
            .props
            .spawn(PropKind::Butterfly, at, z, self.ui, TIMEOUT + 2.0);
        Some(Butterfly {
            fly,
            at,
            way,
            dodge: Vec2::ZERO,
            t: 0.0,
            snap: 0.0,
            cooldown: 0.0,
            phase: Phase::Fly(FLY_SECS),
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_butterfly(
        &mut self,
        mut act: Butterfly,
        dt: f32,
    ) -> Progress<Butterfly> {
        act.left -= dt;
        let gone = matches!(act.phase, Phase::Watch(_));
        if act.left <= 0.0 || (!gone && self.props.get(act.fly).is_none()) {
            return Progress::Dropped;
        }
        act.t += dt;
        let area = self.bounds.nearest(self.body.pos);
        let ui = self.ui;
        match act.phase {
            Phase::Fly(left) => {
                let reach = self.body.pos.distance(act.at);
                let wave = (TAU * WAVE_HZ * act.t).sin() * WAVE;
                let side = Vec2::new(-act.way.y, act.way.x);
                act.at += (act.way + side * wave).normalized() * (SPEED * ui * dt);
                // У края разворачивается внутрь
                let inner = area.clamp(act.at, 40.0 * ui);
                if inner != act.at {
                    act.way = (area.center() - act.at).normalized();
                }
                self.chase(&mut act, reach, dt);
                act.phase = if left > dt {
                    Phase::Fly(left - dt)
                } else {
                    // Улетает к ближнему краю
                    let to = [
                        (act.at.x - area.min.x, Vec2::new(-1.0, 0.0)),
                        (area.max.x - act.at.x, Vec2::new(1.0, 0.0)),
                        (act.at.y - area.min.y, Vec2::new(0.0, -1.0)),
                        (area.max.y - act.at.y, Vec2::new(0.0, 1.0)),
                    ];
                    act.way = to
                        .into_iter()
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map_or(act.way, |a| a.1);
                    Phase::Leave
                };
            }
            Phase::Leave => {
                act.at += act.way * (LEAVE_SPEED * ui * dt);
                self.halt();
                if !area.contains(act.at) {
                    if let Some(fly) = self.props.get_mut(act.fly) {
                        fly.expire_in(0.4);
                    }
                    act.phase = Phase::Watch(WATCH_SECS);
                }
            }
            Phase::Watch(left) => {
                self.antic.neck_up = 1.0;
                self.antic.head_pitch = 0.4;
                if left <= dt {
                    return Progress::Finished;
                }
                act.phase = Phase::Watch(left - dt);
            }
        }
        self.place_butterfly(&act);
        Progress::Going(act)
    }

    /// Бежит за бабочкой, у неё подпрыгивает и щёлкает; та увёртывается.
    fn chase(&mut self, act: &mut Butterfly, reach: f32, dt: f32) {
        let ui = self.ui;
        act.cooldown = (act.cooldown - dt).max(0.0);
        act.dodge = act.dodge * (-DODGE_DECAY * dt).exp();
        act.at += act.dodge * dt;
        if act.snap > 0.0 {
            act.snap = (act.snap - dt).max(0.0);
            let u = 1.0 - act.snap / SNAP_SECS;
            self.antic.lift = 16.0 * (PI * u).sin();
            self.antic.neck_up = (PI * u).sin();
            self.antic.mouth = (1.0 - (2.0 * u - 1.0).abs()).max(0.0);
            return;
        }
        let target = self.bounds.clamp(act.at, self.edge_margin());
        self.brain.push(Directive::RunTo(target));
        if reach <= SNAP_REACH * ui && act.cooldown <= 0.0 {
            act.snap = SNAP_SECS;
            act.cooldown = SNAP_GAP;
            let away = (act.at - self.body.pos).normalized();
            act.dodge = away * (DODGE * ui);
        }
    }

    /// Бабочка в воздухе: высота качается волной.
    fn place_butterfly(&mut self, act: &Butterfly) {
        let z = (HEIGHT[0] + HEIGHT[1] * (TAU * HEIGHT_HZ * act.t).sin()) * self.ui;
        if let Some(fly) = self.props.get_mut(act.fly) {
            fly.pos = act.at;
            fly.z = z;
            fly.vel = Vec2::ZERO;
            fly.vz = 0.0;
        }
    }
}
