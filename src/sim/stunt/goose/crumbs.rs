//! Крошки: горстка лежит свободной кучкой, гусь клюёт её по одной, ближнюю первой.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const COUNT: [usize; 2] = [6, 9];
const SPOT_DISTANCE: [f32; 2] = [120.0, 220.0];
const SPOT_MIN_DISTANCE: f32 = 90.0;
/// Крошки рассыпаны по кругу такого радиуса.
const SPREAD: f32 = 26.0;
/// Нетронутые крошки пропадают сами.
const CRUMB_TTL: f32 = 30.0;
/// Встал на столько логических px от места, откуда клюв достаёт крошку, — клюёт.
const REACH_SLACK: f32 = 9.0;
/// Один клевок и момент удара в нём.
const PECK_SECS: f32 = 0.5;
const STRIKE_SECS: f32 = 0.25;
const TIMEOUT_SECS: f32 = 20.0;
/// Клюв у земли в полном клевке от точки под телом, design px.
pub(in crate::sim::stunt::goose) const PECK_REACH: f32 = 26.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Pick,
    Walk(PropId),
    /// Крошка, куда смотреть, время клевка.
    Peck(PropId, Vec2, f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Crumbs {
    crumbs: Vec<PropId>,
    phase: Phase,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_crumbs(&mut self) -> Option<Crumbs> {
        let spot = self.spot_away(SPOT_DISTANCE, SPOT_MIN_DISTANCE, SPREAD)?;
        let count = COUNT[0] + self.pick(COUNT[1] - COUNT[0] + 1);
        let crumbs = (0..count)
            .map(|_| {
                let angle = self.rng.range(0.0, TAU);
                let reach = SPREAD * self.rng.unit().sqrt() * self.ui;
                let at = spot + Vec2::from_angle(angle) * reach;
                self.props
                    .spawn(PropKind::Crumb, at, 0.0, self.ui, CRUMB_TTL)
            })
            .collect();
        Some(Crumbs {
            crumbs,
            phase: Phase::Pick,
            left: TIMEOUT_SECS,
        })
    }

    pub(in crate::sim::stunt) fn step_crumbs(
        &mut self,
        mut act: Crumbs,
        dt: f32,
    ) -> Progress<Crumbs> {
        act.left -= dt;
        if act.left <= 0.0 {
            self.drop_crumbs(&act);
            return Progress::Dropped;
        }
        // Крошку могли убрать извне
        act.crumbs.retain(|id| self.props.get(*id).is_some());
        act.phase = match act.phase {
            Phase::Pick => match self.nearest_crumb(&act.crumbs) {
                Some(id) => Phase::Walk(id),
                None => {
                    self.halt();
                    self.perform_quietly(self.character.actions().celebrate);
                    return Progress::Finished;
                }
            },
            Phase::Walk(id) => match self.props.get(id).map(|crumb| crumb.pos) {
                None => Phase::Pick,
                Some(at) => {
                    let (face, spot) = self.reach_for(at, PECK_REACH);
                    if self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                        self.halt();
                        Phase::Peck(id, face, 0.0)
                    } else {
                        // Место сдвигается, пока гусь заходит к крошке
                        self.brain.push(Directive::MoveTo(spot));
                        Phase::Walk(id)
                    }
                }
            },
            Phase::Peck(id, face, t) => {
                let t = t + dt;
                self.body.heading = face;
                self.peck_pose((t / PECK_SECS * PI).sin());
                if let Some(crumb) = self.props.get(id).map(|crumb| crumb.pos)
                    && t >= STRIKE_SECS
                {
                    self.props.remove(id);
                    self.burst(Burst::Dust, crumb);
                }
                if t >= PECK_SECS {
                    Phase::Pick
                } else {
                    Phase::Peck(id, face, t)
                }
            }
        };
        Progress::Going(act)
    }

    /// Накладка клевка: шея вперёд-вниз, `depth` 0…1; при 1 кончик клюва
    /// касается земли в `PECK_REACH` перед телом.
    pub(in crate::sim::stunt::goose) fn peck_pose(&mut self, depth: f32) {
        self.antic.neck_forward = 8.0 * depth;
        self.antic.neck_up = -23.0 * depth;
        self.antic.head_pitch = -1.1 * depth;
    }

    fn nearest_crumb(&self, crumbs: &[PropId]) -> Option<PropId> {
        crumbs
            .iter()
            .filter_map(|id| self.props.get(*id).map(|crumb| (*id, crumb.pos)))
            .min_by(|a, b| {
                let from = self.body.pos;
                from.distance(a.1).total_cmp(&from.distance(b.1))
            })
            .map(|(id, _)| id)
    }

    fn drop_crumbs(&mut self, act: &Crumbs) {
        for id in &act.crumbs {
            self.props.remove(*id);
        }
        self.halt();
    }
}
