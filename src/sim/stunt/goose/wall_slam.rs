//! Удар о край: гусь замечает край, разгоняется и не тормозит; врезается, летят перья,
//! сползает по краю, оглушён, потом разглядывает проплешину.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::PropKind;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const NOTICE_SECS: f32 = 0.5;
const RAMP_SECS: f32 = 1.4;
/// Скорость разгона в долях бега: от и до.
const SPEED_FROM: f32 = 0.6;
const SPEED_TO: f32 = 1.5;
/// Ближе края разгоняться негде.
const MIN_RUN: f32 = 150.0;
const PIN_SECS: f32 = 0.2;
const SLIDE_SECS: f32 = 0.5;
const DAZE_SECS: f32 = 1.5;
const INSPECT_SECS: f32 = 2.0;
/// Не долетел до края — сценка сорвалась.
const CHARGE_LIMIT: f32 = 12.0;
const REBOUND_SPEED: f32 = 150.0;
const REBOUND_FRICTION: f32 = 5.0;
const SQUASH: Vec2 = Vec2::new(0.7, 1.15);
const SLIDE_CROUCH: f32 = 0.8;
const FEATHER_HEIGHT: f32 = 30.0;
const FEATHER_TTL: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Notice,
    Charge,
    Pin,
    Slide,
    Daze,
    Inspect,
}

#[derive(Debug)]
pub(in crate::sim) struct WallSlam {
    /// Куда бежит по горизонтали: −1 влево, 1 вправо.
    side: f32,
    phase: Phase,
    /// Время в фазе.
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_wall_slam(&mut self) -> Option<WallSlam> {
        let area = self.bounds.nearest(self.body.pos);
        let (left, right) = (self.body.pos.x - area.min.x, area.max.x - self.body.pos.x);
        let side = if left < right { -1.0 } else { 1.0 };
        if left.min(right) < MIN_RUN * self.ui {
            return None;
        }
        self.halt();
        Some(WallSlam {
            side,
            phase: Phase::Notice,
            t: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_wall_slam(
        &mut self,
        mut act: WallSlam,
        dt: f32,
    ) -> Progress<WallSlam> {
        act.t += dt;
        match act.phase {
            Phase::Notice => {
                self.antic.head_yaw = 0.8;
                self.antic.neck_up = 2.0;
                if act.t >= NOTICE_SECS {
                    let area = self.bounds.nearest(self.body.pos);
                    let edge = if act.side < 0.0 {
                        area.min.x
                    } else {
                        area.max.x
                    };
                    self.brain
                        .push(Directive::RunTo(Vec2::new(edge, self.body.pos.y)));
                    enter(&mut act, Phase::Charge);
                }
            }
            Phase::Charge => {
                if act.t > CHARGE_LIMIT {
                    return self.abandon();
                }
                let ramp = (act.t / RAMP_SECS).min(1.0);
                let run = self.character.motion().run_speed * self.ui;
                let speed = run * (SPEED_FROM + (SPEED_TO - SPEED_FROM) * ramp);
                self.drive = Some(Drive::new(Vec2::new(act.side * speed, 0.0)));
                self.antic.pitch = -0.2;
                self.antic.neck_forward = 5.0;
                if self.bumped.is_some_and(|n| n.x * act.side < 0.0) {
                    self.slam(Vec2::new(-act.side, 0.0));
                    enter(&mut act, Phase::Pin);
                }
            }
            Phase::Pin => {
                self.antic.squash = SQUASH;
                if act.t >= PIN_SECS {
                    let away = Vec2::new(-act.side, 0.0);
                    self.drive = Some(
                        Drive::new(away * (REBOUND_SPEED * self.ui)).friction(REBOUND_FRICTION),
                    );
                    enter(&mut act, Phase::Slide);
                }
            }
            Phase::Slide => {
                let k = (act.t / SLIDE_SECS).min(1.0);
                self.antic.crouch = SLIDE_CROUCH * k;
                self.antic.squash = SQUASH.lerp(Vec2::new(1.0, 1.0), k);
                if act.t >= SLIDE_SECS {
                    enter(&mut act, Phase::Daze);
                }
            }
            Phase::Daze => {
                self.antic.crouch = SLIDE_CROUCH * (1.0 - act.t / DAZE_SECS).max(0.0);
                if act.t >= DAZE_SECS {
                    enter(&mut act, Phase::Inspect);
                }
            }
            Phase::Inspect => {
                self.antic.head_yaw = 1.4;
                self.antic.neck_side = 3.0;
                self.antic.eyes = Eyes::Wide;
                if act.t >= INSPECT_SECS {
                    self.drive = None;
                    self.halt();
                    return Progress::Finished;
                }
            }
        }
        Progress::Going(act)
    }

    fn abandon(&mut self) -> Progress<WallSlam> {
        self.drive = None;
        self.halt();
        Progress::Dropped
    }

    /// Удар: перья, звёзды, оглушён и лысина.
    fn slam(&mut self, away: Vec2) {
        self.drive = Some(Drive::new(Vec2::ZERO));
        self.halt();
        let (body, head) = (self.body.pos, self.head());
        for at in [body, head] {
            self.burst(Burst::Feather, at);
        }
        self.burst(Burst::Stars, head);
        for _ in 0..2 + self.pick(2) {
            self.shed_feather(head, away);
        }
        self.cond.dazed = 1.0;
        self.cond.bald = 1.0;
    }

    /// Перо слетает с `at` в сторону `away` и плавно оседает.
    pub(in crate::sim::stunt) fn shed_feather(&mut self, at: Vec2, away: Vec2) {
        let ui = self.ui;
        let id = self
            .props
            .spawn(PropKind::Feather, at, FEATHER_HEIGHT * ui, ui, FEATHER_TTL);
        let speed = self.rng.range(30.0, 90.0) * ui;
        let (lift, drift, spin) = (
            self.rng.range(60.0, 160.0) * ui,
            self.rng.range(-40.0, 40.0) * ui,
            self.rng.range(-4.0, 4.0),
        );
        if let Some(prop) = self.props.get_mut(id) {
            prop.vel = Vec2::new(away.x * speed, drift);
            prop.vz = lift;
            prop.spin = spin;
        }
    }
}

fn enter(act: &mut WallSlam, phase: Phase) {
    act.phase = phase;
    act.t = 0.0;
}
