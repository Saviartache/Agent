//! Тычок: подходит к курсору и тыкает его клювом или рылом; искры, отскок. Иногда
//! промахивается: выпад в пустоту, кувырок вперёд, оглушён. Курсор не двигается.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const APPROACH_SECS: f32 = 8.0;
/// Голова так близко к курсору — пора тыкать.
const REACH: f32 = 24.0;
const MISS_CHANCE: f32 = 0.25;
const POKE_SECS: f32 = 0.25;
const POKE_NECK: f32 = 22.0;
const RECOIL_SPEED: f32 = 90.0;
const RECOIL_FRICTION: f32 = 5.0;
const RECOIL_SECS: f32 = 0.5;
const RECOIL_HOP: f32 = 8.0;
const TUMBLE_SECS: f32 = 0.6;
const TUMBLE_SPEED: f32 = 200.0;
const TUMBLE_FRICTION: f32 = 3.0;
const TUMBLE_HOP: f32 = 10.0;
const DAZE_SECS: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Approach { left: f32 },
    Poke { t: f32, miss: bool },
    Recoil { t: f32 },
    Tumble { t: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Boop {
    phase: Phase,
    /// Куда смотрел при тычке: отскок не разворачивает тело.
    face: Vec2,
    spot: Vec2,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_boop(&mut self) -> Option<Boop> {
        let cursor = self.cursor?;
        self.brain.push(Directive::MoveTo(cursor));
        Some(Boop {
            phase: Phase::Approach {
                left: APPROACH_SECS,
            },
            face: self.body.heading,
            spot: cursor,
        })
    }

    pub(in crate::sim::stunt) fn step_boop(&mut self, mut act: Boop, dt: f32) -> Progress<Boop> {
        match act.phase {
            Phase::Approach { left } => {
                let Some(cursor) = self.cursor.filter(|_| left > dt) else {
                    self.halt();
                    return Progress::Dropped;
                };
                let near = self
                    .head()
                    .distance(cursor)
                    .min(self.body.pos.distance(cursor));
                if near > REACH * self.ui {
                    act.phase = Phase::Approach { left: left - dt };
                    return Progress::Going(act);
                }
                self.halt();
                self.drive = Some(Drive::new(Vec2::ZERO));
                act.face = self.body.heading;
                act.spot = cursor;
                act.phase = Phase::Poke {
                    t: 0.0,
                    miss: self.rng.chance(MISS_CHANCE),
                };
            }
            Phase::Poke { t, miss } => {
                let t = t + dt;
                self.boop_pose(t, miss);
                if t < POKE_SECS {
                    act.phase = Phase::Poke { t, miss };
                    return Progress::Going(act);
                }
                act.phase = if miss {
                    self.drive = Some(
                        Drive::new(act.face * (TUMBLE_SPEED * self.ui)).friction(TUMBLE_FRICTION),
                    );
                    Phase::Tumble { t: 0.0 }
                } else {
                    self.burst(Burst::Sparks, act.spot);
                    self.drive = Some(
                        Drive::new(-act.face * (RECOIL_SPEED * self.ui)).friction(RECOIL_FRICTION),
                    );
                    Phase::Recoil { t: 0.0 }
                };
            }
            Phase::Recoil { t } => {
                let t = t + dt;
                self.antic.lift = RECOIL_HOP * (PI * t / RECOIL_SECS).sin().max(0.0);
                self.keep_facing_driven(act.face);
                if t >= RECOIL_SECS {
                    return self.boop_finish(act.face);
                }
                act.phase = Phase::Recoil { t };
            }
            Phase::Tumble { t } => {
                let t = t + dt;
                self.antic.roll = TAU * (t / TUMBLE_SECS).min(1.0);
                self.antic.lift = TUMBLE_HOP * (PI * t / TUMBLE_SECS).sin().max(0.0);
                self.antic.eyes = Eyes::Wide;
                if t >= TUMBLE_SECS + DAZE_SECS {
                    return self.boop_finish(act.face);
                }
                if t >= TUMBLE_SECS {
                    self.cond.dazed = 0.5;
                }
                act.phase = Phase::Tumble { t };
            }
        }
        Progress::Going(act)
    }

    fn boop_finish(&mut self, face: Vec2) -> Progress<Boop> {
        self.body.heading = face;
        self.halt();
        Progress::Finished
    }

    /// Шея вперёд-вниз и обратно; промах — с круглыми глазами.
    fn boop_pose(&mut self, t: f32, miss: bool) {
        let strike = (PI * (t / POKE_SECS).min(1.0)).sin();
        self.antic.neck_forward = POKE_NECK * strike;
        self.antic.neck_up = -5.0 * strike;
        self.antic.head_pitch = -0.3 * strike;
        if miss {
            self.antic.eyes = Eyes::Wide;
        }
    }

    /// Тело едет, но смотрит туда, куда смотрело: поправка к повороту по скорости.
    pub(in crate::sim::stunt) fn keep_facing_driven(&mut self, face: Vec2) {
        let angle = |v: Vec2| v.y.atan2(v.x);
        let delta = angle(face) - angle(self.body.heading);
        self.antic.spin += (delta + PI).rem_euclid(TAU) - PI;
    }
}
