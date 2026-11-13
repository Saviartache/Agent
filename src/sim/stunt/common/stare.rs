//! Гляделки: идёт к середине монитора, поворачивается лицом к зрителю и долго смотрит,
//! голова следит за курсором. Потом резко отворачивается и уходит.

use std::f32::consts::PI;

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, время — с.
const WALK_SECS: f32 = 10.0;
const ARRIVED: f32 = 40.0;
const TURN_SECS: f32 = 0.6;
const STARE_SECS: f32 = 6.0;
const SNAP_SECS: f32 = 0.3;
const LEAVE: [f32; 2] = [150.0, 300.0];
const MAX_YAW: f32 = 1.2;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk { left: f32 },
    Stare { t: f32 },
    Snap { t: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Stare {
    phase: Phase,
    centre: Vec2,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_stare(&mut self) -> Option<Stare> {
        let area = self.bounds.nearest(self.body.pos);
        let centre = area.clamp(area.center(), self.edge_margin());
        self.brain.push(Directive::MoveTo(centre));
        Some(Stare {
            phase: Phase::Walk { left: WALK_SECS },
            centre,
        })
    }

    pub(in crate::sim::stunt) fn step_stare(&mut self, mut act: Stare, dt: f32) -> Progress<Stare> {
        match act.phase {
            Phase::Walk { left } => {
                let near = self.body.pos.distance(act.centre) <= ARRIVED * self.ui;
                act.phase = if near || left <= dt {
                    self.halt();
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    Phase::Stare { t: 0.0 }
                } else {
                    Phase::Walk { left: left - dt }
                };
            }
            Phase::Stare { t } => {
                let t = t + dt;
                self.antic.spin = self.stare_face_viewer() * (t / TURN_SECS).min(1.0);
                self.antic.head_yaw = self.stare_watch_cursor();
                act.phase = if t >= STARE_SECS {
                    Phase::Snap { t: 0.0 }
                } else {
                    Phase::Stare { t }
                };
            }
            Phase::Snap { t } => {
                let t = t + dt;
                self.antic.spin = self.stare_face_viewer() + PI * (t / SNAP_SECS).min(1.0);
                if t >= SNAP_SECS {
                    self.drive = None;
                    if let Some(spot) = self.spot_away(LEAVE, LEAVE[0] * 0.5, 20.0) {
                        self.brain.push(Directive::MoveTo(spot));
                    }
                    return Progress::Finished;
                }
                act.phase = Phase::Snap { t };
            }
        }
        Progress::Going(act)
    }

    /// Поворот тела от нынешнего хода лицом вниз экрана.
    fn stare_face_viewer(&self) -> f32 {
        let turn = PI * 0.5 - self.body.heading.y.atan2(self.body.heading.x);
        (turn + PI).rem_euclid(2.0 * PI) - PI
    }

    /// Куда повернуть голову от лица тела, чтобы смотреть на курсор.
    fn stare_watch_cursor(&self) -> f32 {
        let Some(cursor) = self.cursor else {
            return 0.0;
        };
        let to = cursor - self.body.pos;
        let turn = to.y.atan2(to.x) - PI * 0.5;
        ((turn + PI).rem_euclid(2.0 * PI) - PI).clamp(-MAX_YAW, MAX_YAW)
    }
}
