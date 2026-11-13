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
/// Подошёл на столько design px к крошке — клюёт.
const REACH: f32 = 20.0;
/// Один клевок и момент удара в нём.
const PECK_SECS: f32 = 0.5;
const STRIKE_SECS: f32 = 0.25;
const TIMEOUT_SECS: f32 = 20.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Pick,
    Walk(PropId),
    Peck(PropId, f32),
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
                Some((id, at)) => {
                    self.brain.push(Directive::MoveTo(at));
                    Phase::Walk(id)
                }
                None => {
                    self.halt();
                    self.perform_quietly(self.character.actions().celebrate);
                    return Progress::Finished;
                }
            },
            Phase::Walk(id) => match self.props.get(id).map(|crumb| crumb.pos) {
                None => Phase::Pick,
                Some(at) if self.body.pos.distance(at) <= REACH * self.sprite => {
                    self.halt();
                    Phase::Peck(id, 0.0)
                }
                Some(_) => Phase::Walk(id),
            },
            Phase::Peck(id, t) => {
                let t = t + dt;
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
                    Phase::Peck(id, t)
                }
            }
        };
        Progress::Going(act)
    }

    /// Накладка клевка: шея вперёд-вниз, `depth` 0…1.
    pub(in crate::sim::stunt::goose) fn peck_pose(&mut self, depth: f32) {
        self.antic.neck_forward = 9.0 * depth;
        self.antic.neck_up = -5.0 * depth;
        self.antic.head_pitch = -0.9 * depth;
    }

    fn nearest_crumb(&self, crumbs: &[PropId]) -> Option<(PropId, Vec2)> {
        crumbs
            .iter()
            .filter_map(|id| self.props.get(*id).map(|crumb| (*id, crumb.pos)))
            .min_by(|a, b| {
                let from = self.body.pos;
                from.distance(a.1).total_cmp(&from.distance(b.1))
            })
    }

    fn drop_crumbs(&mut self, act: &Crumbs) {
        for id in &act.crumbs {
            self.props.remove(*id);
        }
        self.halt();
    }
}
