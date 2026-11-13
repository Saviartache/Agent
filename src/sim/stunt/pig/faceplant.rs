//! Носом: спотыкается на бегу и пашет пятачком борозду.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, рад, с.
const RUN_DISTANCE: [f32; 2] = [230.0, 270.0];
const RUN_MIN: f32 = 150.0;
const RUN_SECS: f32 = 8.0;
/// Спотыкается на этой доле пути.
const TRIP_AT: f32 = 0.5;
const PLOW_FRICTION: f32 = 3.0;
const PLOW_MAX_SECS: f32 = 2.5;
/// Медленнее этого — остановилась, логические px/с.
const STOPPED: f32 = 12.0;
const PLOW_GAP: f32 = 0.1;
const PITCH: f32 = -1.0;
const HEAD_PITCH: f32 = -0.8;
const LIE_SECS: f32 = 0.7;
const RISE_SECS: f32 = 0.6;
const LOOK_SECS: f32 = 2.0;
const LOOK_SWING: f32 = 0.9;
const MUD_WEIGHT: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run,
    Plow { gap: f32 },
    Lie,
    Rise,
    Look,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Faceplant {
    from: Vec2,
    to: Vec2,
    phase: Phase,
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_faceplant(&mut self) -> Option<Faceplant> {
        let to = self.spot_away(RUN_DISTANCE, RUN_MIN, 40.0)?;
        self.brain.push(Directive::RunTo(to));
        Some(Faceplant {
            from: self.body.pos,
            to,
            phase: Phase::Run,
            t: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_faceplant(
        &mut self,
        mut act: Faceplant,
        dt: f32,
    ) -> Progress<Faceplant> {
        act.t += dt;
        let t = act.t;
        let mut next = None;
        match act.phase {
            Phase::Run => {
                let covered = self.body.pos.distance(act.from) / act.from.distance(act.to);
                if covered >= TRIP_AT {
                    self.halt();
                    self.drive = Some(Drive::new(self.body.vel).friction(PLOW_FRICTION));
                    self.cond.snout = 1.0;
                    next = Some(Phase::Plow { gap: 0.0 });
                } else if t > RUN_SECS {
                    return Progress::Dropped;
                }
            }
            Phase::Plow { gap } => {
                self.dive();
                let gap = if gap <= 0.0 {
                    let snout = self.head();
                    self.burst(Burst::Dirt, snout);
                    self.mud_prints
                        .stamp(snout, self.body.heading, true, 0.0, MUD_WEIGHT);
                    PLOW_GAP
                } else {
                    gap - dt
                };
                act.phase = Phase::Plow { gap };
                let slow = self.body.vel.length() < STOPPED * self.ui;
                if slow || t > PLOW_MAX_SECS {
                    self.drive = None;
                    self.halt();
                    next = Some(Phase::Lie);
                }
            }
            Phase::Lie => {
                self.dive();
                if t >= LIE_SECS {
                    next = Some(Phase::Rise);
                }
            }
            Phase::Rise => {
                let up = 1.0 - (t / RISE_SECS).min(1.0);
                self.antic.pitch = PITCH * up;
                self.antic.neck_forward = up;
                self.antic.head_pitch = HEAD_PITCH * up;
                if t >= RISE_SECS {
                    next = Some(Phase::Look);
                }
            }
            Phase::Look => {
                self.antic.head_yaw = LOOK_SWING * (TAU * t / LOOK_SECS * 1.5).sin();
                if t >= LOOK_SECS {
                    return Progress::Finished;
                }
            }
        }
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }

    /// Нос в землю.
    fn dive(&mut self) {
        self.antic.pitch = PITCH;
        self.antic.neck_forward = 1.0;
        self.antic.neck_up = -0.5;
        self.antic.head_pitch = HEAD_PITCH;
    }
}
