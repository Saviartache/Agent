//! Мышка! Замечает курсор, глаза круглые, привстаёт на задние ноги с поднятым хоботом
//! и хлопающими ушами, пятится от него и, отдышавшись, косится. Без курсора — отказ.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с, частоты — Гц.
const NOTICE_RANGE: f32 = 400.0;
const FREEZE_SECS: f32 = 0.4;
const REAR_SECS: f32 = 1.4;
const BACK_SECS: f32 = 0.8;
const CALM_SECS: f32 = 1.0;
const BACK_SPEED: f32 = 120.0;
const BACK_FRICTION: f32 = 3.0;
/// Привстаёт: нос вверх, тело чуть над землёй — передние ноги отрываются сами.
const REAR_PITCH: f32 = 0.75;
const REAR_LIFT: f32 = 1.4;
const TREMBLE_HZ: f32 = 14.0;
const FLAP_HZ: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct MouseScare {
    t: f32,
    mouse: Vec2,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_mouse_scare(&mut self) -> Option<MouseScare> {
        let mouse = self.cursor?;
        if mouse.distance(self.body.pos) > NOTICE_RANGE * self.ui {
            return None;
        }
        self.halt();
        Some(MouseScare { t: 0.0, mouse })
    }

    pub(in crate::sim::stunt) fn step_mouse_scare(
        &mut self,
        mut act: MouseScare,
        dt: f32,
    ) -> Progress<MouseScare> {
        let was = act.t;
        act.t += dt;
        let t = act.t;
        let to = act.mouse - self.body.pos;
        if to != Vec2::ZERO {
            self.body.heading = to.normalized();
        }
        let rear_at = FREEZE_SECS;
        let back_at = rear_at + REAR_SECS;
        let calm_at = back_at + BACK_SECS;
        if t < rear_at {
            self.antic.eyes = Eyes::Wide;
            self.antic.wobble = 0.4 * (t * TAU * TREMBLE_HZ).sin();
        } else if t < back_at {
            if was < rear_at {
                self.burst(Burst::Alert, self.head());
            }
            let up = (PI * (t - rear_at) / REAR_SECS).sin().max(0.0).sqrt();
            self.rear(up, t);
        } else if t < calm_at {
            if was < back_at {
                let away = -to.normalized();
                self.drive =
                    Some(Drive::new(away * (BACK_SPEED * self.ui)).friction(BACK_FRICTION));
            }
            self.antic.eyes = Eyes::Wide;
            self.antic.crouch = 1.5;
        } else if t < calm_at + CALM_SECS {
            if was < calm_at {
                self.drive = None;
                self.halt();
            }
            // Косится на «мышку» и выдыхает
            self.antic.head_yaw = 0.5 * (PI * (t - calm_at) / CALM_SECS).sin();
            self.antic.lids = 0.3;
        } else {
            return Progress::Finished;
        }
        Progress::Going(act)
    }

    /// Привстал на задние ноги, хобот вверх, уши веером; `up` 0…1.
    fn rear(&mut self, up: f32, t: f32) {
        self.antic.pitch = REAR_PITCH * up;
        self.antic.lift = REAR_LIFT * up;
        self.antic.head_pitch = 0.9 * up;
        self.antic.mouth = up;
        self.antic.eyes = Eyes::Wide;
        self.antic.wings = up;
        self.antic.flap = (t * FLAP_HZ).fract();
        self.antic.wobble = 0.5 * up * (t * TAU * TREMBLE_HZ).sin();
    }
}
