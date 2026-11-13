//! Узел: тянется шеей к курсору, шея завязывается узлом, гусь распутывается, крутясь,
//! и узел развязывается с «чпок». Курсор не трогаем.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
/// Подходит к курсору на столько, чтобы дотянуться шеей.
const CLOSE: f32 = 70.0;
const APPROACH_SECS: f32 = 5.0;
const STRETCH_SECS: f32 = 1.2;
const REACH_FORWARD: f32 = 14.0;
const REACH_UP: f32 = 4.0;
const KNOT_SECS: f32 = 0.6;
const TWIST_SECS: f32 = 1.5;
const TWIST_ANGLE: f32 = 0.9;
const POP_SECS: f32 = 0.35;
const POP_HOP: f32 = 8.0;
const SHAKE_SECS: f32 = 0.7;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Approach(f32),
    Stretch(f32),
    Knot(f32),
    Twist(f32),
    Pop(f32),
    Shake(f32),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct NeckTangle {
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_neck_tangle(&mut self) -> Option<NeckTangle> {
        let cursor = self.cursor?;
        self.brain.push(Directive::MoveTo(cursor));
        Some(NeckTangle {
            phase: Phase::Approach(APPROACH_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_neck_tangle(
        &mut self,
        act: NeckTangle,
        dt: f32,
    ) -> Progress<NeckTangle> {
        let phase = match act.phase {
            Phase::Approach(left) => {
                // Курсор ушёл — сценка пропала
                let Some(cursor) = self.cursor else {
                    self.halt();
                    return Progress::Dropped;
                };
                if self.body.pos.distance(cursor) <= CLOSE * self.ui {
                    self.halt();
                    Phase::Stretch(0.0)
                } else if left <= dt {
                    self.halt();
                    return Progress::Dropped;
                } else {
                    Phase::Approach(left - dt)
                }
            }
            Phase::Stretch(t) => {
                let t = t + dt;
                let k = (t / STRETCH_SECS).min(1.0);
                self.reach(k);
                if t < STRETCH_SECS {
                    Phase::Stretch(t)
                } else {
                    Phase::Knot(0.0)
                }
            }
            Phase::Knot(t) => {
                let t = t + dt;
                self.reach(1.0);
                self.antic.knot = (t / KNOT_SECS).min(1.0);
                self.antic.eyes = Eyes::Crossed;
                if t < KNOT_SECS {
                    Phase::Knot(t)
                } else {
                    Phase::Twist(0.0)
                }
            }
            Phase::Twist(t) => {
                let t = t + dt;
                self.reach(1.0);
                self.antic.knot = 1.0;
                self.antic.eyes = Eyes::Crossed;
                self.antic.spin = TWIST_ANGLE * (t * TAU * 1.5).sin();
                if t < TWIST_SECS {
                    Phase::Twist(t)
                } else {
                    self.burst(Burst::Sparks, self.head());
                    Phase::Pop(0.0)
                }
            }
            Phase::Pop(t) => {
                let t = t + dt;
                self.antic.lift = POP_HOP * (t / POP_SECS * PI).sin();
                if t < POP_SECS {
                    Phase::Pop(t)
                } else {
                    Phase::Shake(0.0)
                }
            }
            Phase::Shake(t) => {
                let t = t + dt;
                if t >= SHAKE_SECS {
                    return Progress::Finished;
                }
                self.antic.wobble = 1.4 * (1.0 - t / SHAKE_SECS) * (t * TAU * 11.0).sin();
                Phase::Shake(t)
            }
        };
        Progress::Going(NeckTangle { phase })
    }

    /// Шея тянется вперёд и вверх, к курсору; `k` 0…1.
    fn reach(&mut self, k: f32) {
        self.antic.neck_forward = REACH_FORWARD * k;
        self.antic.neck_up = REACH_UP * k;
    }
}
