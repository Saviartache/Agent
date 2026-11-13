//! Потягушка: поклон носом вниз, зевок с поднятой шеей, встряска головой.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с.
const BOW_END: f32 = 1.5;
const YAWN_END: f32 = 3.0;
const SECS: f32 = 3.5;
const RAMP: f32 = 0.4;
const SHAKE_HZ: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Stretch {
    t: f32,
}

/// Плавно вошёл в `[from, to]` и вышел из него, `0…1`.
fn window(t: f32, from: f32, to: f32) -> f32 {
    ((t - from) / RAMP).min((to - t) / RAMP).clamp(0.0, 1.0)
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_stretch(&mut self) -> Option<Stretch> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Stretch { t: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_stretch(
        &mut self,
        mut act: Stretch,
        dt: f32,
    ) -> Progress<Stretch> {
        act.t += dt;
        if act.t >= SECS {
            self.halt();
            return Progress::Finished;
        }
        let t = act.t;
        let bow = window(t, 0.0, BOW_END);
        self.antic.pitch = -0.25 * bow;
        self.antic.neck_forward = 8.0 * bow;
        self.antic.head_pitch = -0.3 * bow;
        self.antic.squash = Vec2::new(1.0 + 0.15 * bow, 1.0 - 0.1 * bow);
        let yawn = window(t, BOW_END, YAWN_END);
        self.antic.mouth = yawn;
        self.antic.lids = 0.6 * yawn;
        self.antic.neck_up = 4.0 * yawn;
        self.antic.head_pitch += 0.5 * yawn;
        let shake = window(t, YAWN_END, SECS) * (TAU * SHAKE_HZ * t).sin();
        self.antic.head_yaw = 0.5 * shake;
        self.antic.head_roll = 0.3 * shake;
        Progress::Going(act)
    }
}
