//! Отряхивается: грязные брызги, грязи вдвое меньше.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Intent;
use crate::command::Stunt;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Грязнее этого — просится отряхнуться сама.
const SELF_MUDDY: f32 = 0.6;
/// Грязнее этого отряхиваться есть смысл.
const MIN_MUDDY: f32 = 0.15;
/// В среднем раз в столько секунд.
const SELF_EVERY: f32 = 20.0;
const SECS: f32 = 1.6;
const SPLASHES: u32 = 4;
const WOBBLE_HZ: f32 = 12.0;
const WOBBLE: f32 = 3.0;
const HEAD_HZ: f32 = 6.0;
const HEAD_SWING: f32 = 0.7;
/// Разлёт брызг, логические px.
const SPRAY: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct ShakeOff {
    t: f32,
    splashed: u32,
    muddy: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_shake_off(&mut self) -> Option<ShakeOff> {
        if self.cond.muddy <= MIN_MUDDY {
            return None;
        }
        self.halt();
        Some(ShakeOff {
            t: 0.0,
            splashed: 0,
            muddy: self.cond.muddy,
        })
    }

    pub(in crate::sim::stunt) fn step_shake_off(
        &mut self,
        mut act: ShakeOff,
        dt: f32,
    ) -> Progress<ShakeOff> {
        act.t += dt;
        self.antic.wobble = WOBBLE * (TAU * WOBBLE_HZ * act.t).sin();
        self.antic.head_yaw = HEAD_SWING * (TAU * HEAD_HZ * act.t).sin();
        self.antic.lids = 0.6;
        let due = (act.t / SECS * SPLASHES as f32) as u32;
        while act.splashed < due.min(SPLASHES) {
            act.splashed += 1;
            let around = Vec2::new(self.rng.range(-SPRAY, SPRAY), self.rng.range(-SPRAY, SPRAY));
            self.burst(Burst::Mud, self.body.pos + around * self.ui);
        }
        self.cond.muddy = act.muddy * (1.0 - 0.5 * (act.t / SECS).min(1.0));
        if act.t >= SECS {
            return Progress::Finished;
        }
        Progress::Going(act)
    }

    /// Очень грязная свинка без дела время от времени отряхивается сама.
    pub(in crate::sim) fn shake_off_reflex(&mut self, dt: f32) {
        let idle = matches!(self.brain.intent(), Intent::Wander | Intent::Rest);
        if self.cond.muddy > SELF_MUDDY
            && self.act.is_none()
            && idle
            && self.rng.chance(dt / SELF_EVERY)
        {
            self.stage(Stunt::ShakeOff);
        }
    }
}
