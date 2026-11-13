//! Поскользнулся: на ходу лапы уезжают, пингвин балансирует, размахивая ластами,
//! и шлёпается ничком; полежав и побарахтавшись, встаёт оглушённым.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, px/с, 1/с, с; амплитуды — design px и радианы.
const DISTANCE: [f32; 2] = [200.0, 320.0];
const MIN_DISTANCE: f32 = 150.0;
const WALK_SECS: f32 = 1.2;
const SKID_SPEED: f32 = 160.0;
const SKID_FRICTION: f32 = 2.5;
const TEETER_SECS: f32 = 0.9;
const TEETER_HZ: f32 = 3.0;
const TEETER_PITCH: f32 = 0.35;
const TEETER_WOBBLE: f32 = 2.5;
const FLAP_HZ: f32 = 5.0;
const FALL_SECS: f32 = 0.25;
const HOP: f32 = 8.0;
const LIE_SECS: f32 = 1.4;
const RISE_SECS: f32 = 0.6;
const TIMEOUT: f32 = 12.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk,
    Teeter,
    Fall,
    Lie,
    Rise,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Wipeout {
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_wipeout(&mut self) -> Option<Wipeout> {
        let to = self.spot_away(DISTANCE, MIN_DISTANCE, 40.0)?;
        self.brain.push(Directive::MoveTo(to));
        Some(Wipeout {
            phase: Phase::Walk,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_wipeout(
        &mut self,
        mut act: Wipeout,
        dt: f32,
    ) -> Progress<Wipeout> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            self.halt();
            return Progress::Dropped;
        }
        let next = match act.phase {
            Phase::Walk => (act.t >= WALK_SECS).then(|| {
                let dir = match self.body.vel.normalized() {
                    v if v.length() > 0.0 => v,
                    _ => self.body.heading,
                };
                self.halt();
                let vel = dir * (SKID_SPEED * self.ui);
                self.drive = Some(Drive::new(vel).friction(SKID_FRICTION));
                Phase::Teeter
            }),
            Phase::Teeter => {
                // Откидывается назад и ловит равновесие ластами
                let sway = (TAU * TEETER_HZ * act.t).sin();
                self.antic.pitch = TEETER_PITCH * sway.abs();
                self.antic.wobble = TEETER_WOBBLE * sway;
                self.flail(act.t);
                (act.t >= TEETER_SECS).then_some(Phase::Fall)
            }
            Phase::Fall => {
                let u = (act.t / FALL_SECS).min(1.0);
                self.prone(u);
                self.antic.lift = HOP * (PI * u).sin();
                self.antic.wings = 1.0;
                (act.t >= FALL_SECS).then(|| {
                    self.burst(Burst::Dust, self.body.pos);
                    self.burst(Burst::Stars, self.head());
                    Phase::Lie
                })
            }
            Phase::Lie => {
                self.prone(1.0);
                self.flail(act.t);
                (act.t >= LIE_SECS).then(|| {
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    Phase::Rise
                })
            }
            Phase::Rise => {
                self.prone(1.0 - (act.t / RISE_SECS).min(1.0));
                if act.t >= RISE_SECS {
                    self.cond.dazed = 1.0;
                    return Progress::Finished;
                }
                None
            }
        };
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }

    /// Машет ластами что есть сил.
    fn flail(&mut self, t: f32) {
        self.antic.wings = 1.0;
        self.antic.flap = (FLAP_HZ * t).fract();
    }
}
