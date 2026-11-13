//! Танец: переступает в ритм, качает головой и телом, поворачивается туда-сюда;
//! ноты летят через каждые две доли.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с; амплитуды — design px и радианы.
const SECS: f32 = 6.0;
const BEAT: f32 = 0.5;
/// Плавный вход и выход из танца.
const EASE_SECS: f32 = 0.4;
const WOBBLE: f32 = 3.0;
const HEAD_ROLL: f32 = 0.25;
const SPIN: f32 = 0.3;
const STRONG_HOP: f32 = 5.0;
const WEAK_HOP: f32 = 2.5;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Dance {
    t: f32,
    notes: u32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_dance(&mut self) -> Option<Dance> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Dance { t: 0.0, notes: 0 })
    }

    pub(in crate::sim::stunt) fn step_dance(&mut self, mut act: Dance, dt: f32) -> Progress<Dance> {
        act.t += dt;
        if act.t >= SECS {
            self.halt();
            return Progress::Finished;
        }
        let ease = (act.t / EASE_SECS).min((SECS - act.t) / EASE_SECS).min(1.0);
        let beat = act.t / BEAT;
        let pair = TAU * act.t / (2.0 * BEAT);
        let hop = if (beat as u32).is_multiple_of(2) {
            STRONG_HOP
        } else {
            WEAK_HOP
        };
        self.antic.lift = ease * hop * (PI * beat.fract()).sin();
        self.antic.wobble = ease * WOBBLE * pair.sin();
        self.antic.head_roll = ease * HEAD_ROLL * pair.cos();
        self.antic.spin = ease * SPIN * (pair * 0.5).sin();
        if beat as u32 / 2 >= act.notes {
            act.notes += 1;
            self.burst(Burst::Notes, self.head());
        }
        Progress::Going(act)
    }
}
