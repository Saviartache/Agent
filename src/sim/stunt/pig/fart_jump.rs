//! Пуф: облачко сзади подбрасывает свинку, она обнюхивает облако.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Логические px, design px, рад, с.
const WAIT_SECS: f32 = 0.3;
const BEHIND: f32 = 26.0;
const CLOUD_RAISE: f32 = 8.0;
const JUMP_HEIGHT: f32 = 20.0;
const JUMP_SECS: f32 = 0.4;
const TURN_SECS: f32 = 0.5;
const SNIFF_SECS: f32 = 1.2;
const SNIFF_HZ: f32 = 3.0;
const RECOIL_SECS: f32 = 0.8;
const LEAVE_SECS: f32 = 0.6;
const LEAVE_DISTANCE: [f32; 2] = [140.0, 240.0];

#[derive(Clone, Copy, Debug)]
enum Phase {
    Wait,
    Jump,
    Turn,
    Sniff,
    Recoil,
    Leave,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct FartJump {
    phase: Phase,
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_fart_jump(&mut self) -> Option<FartJump> {
        self.halt();
        Some(FartJump {
            phase: Phase::Wait,
            t: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_fart_jump(
        &mut self,
        mut act: FartJump,
        dt: f32,
    ) -> Progress<FartJump> {
        act.t += dt;
        let t = act.t;
        let mut next = None;
        match act.phase {
            Phase::Wait => {
                if t >= WAIT_SECS {
                    let behind = self.body.pos - self.body.heading * (BEHIND * self.ui);
                    self.burst(Burst::Whiff, behind - Vec2::new(0.0, CLOUD_RAISE * self.ui));
                    next = Some(Phase::Jump);
                }
            }
            Phase::Jump => {
                self.antic.lift = JUMP_HEIGHT * (PI * t / JUMP_SECS).sin().max(0.0);
                self.antic.eyes = Eyes::Wide;
                if t >= JUMP_SECS {
                    next = Some(Phase::Turn);
                }
            }
            Phase::Turn => {
                self.antic.spin = PI * (t / TURN_SECS).min(1.0);
                if t >= TURN_SECS {
                    next = Some(Phase::Sniff);
                }
            }
            Phase::Sniff => {
                self.antic.spin = PI;
                self.antic.neck_forward = 1.0;
                self.antic.head_pitch = -0.3 + 0.1 * (TAU * SNIFF_HZ * t).sin();
                if t >= SNIFF_SECS {
                    next = Some(Phase::Recoil);
                }
            }
            Phase::Recoil => {
                self.antic.spin = PI;
                self.antic.neck_up = 1.0;
                self.antic.neck_forward = -0.5;
                self.antic.head_pitch = 0.3;
                self.antic.lids = 0.5;
                if t >= RECOIL_SECS {
                    let away = self.spot_away(LEAVE_DISTANCE, 0.0, 0.0);
                    self.brain
                        .push(Directive::MoveTo(away.unwrap_or(self.body.pos)));
                    next = Some(Phase::Leave);
                }
            }
            Phase::Leave => {
                if t >= LEAVE_SECS {
                    self.halt();
                    return Progress::Finished;
                }
            }
        }
        if let Some(phase) = next {
            act = FartJump { phase, t: 0.0 };
        }
        Progress::Going(act)
    }
}
