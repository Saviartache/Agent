//! Трубит: набирает воздух, задирает хобот и трубит раз за разом — из кончика
//! нотки и волны, уши хлопают.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::sim::Sim;

/// Время — с, частоты — Гц.
const INHALE_SECS: f32 = 0.6;
const TOOT_SECS: f32 = 0.9;
const NOTE_GAP: f32 = 0.22;
const RING_GAP: f32 = 0.3;
const SETTLE_SECS: f32 = 0.6;
const FLAP_HZ: f32 = 3.0;
/// Хобот вверх кивком головы, рад; на вдохе голова чуть опущена.
const RAISE: f32 = 1.05;
const INHALE_DIP: f32 = -0.2;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Trumpet {
    t: f32,
    toots: u8,
    note: f32,
    ring: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_trumpet(&mut self) -> Option<Trumpet> {
        self.halt();
        Some(Trumpet {
            t: 0.0,
            toots: 2 + self.pick(2) as u8,
            note: 0.0,
            ring: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_trumpet(
        &mut self,
        mut act: Trumpet,
        dt: f32,
    ) -> Progress<Trumpet> {
        act.t += dt;
        self.face_sideways();
        let blow = act.t - INHALE_SECS;
        let total = f32::from(act.toots) * TOOT_SECS;
        if blow < 0.0 {
            let k = act.t / INHALE_SECS;
            self.antic.head_pitch = INHALE_DIP * (PI * k).sin();
            self.antic.squash = crate::math::Vec2::new(1.0 + 0.08 * k, 1.0 + 0.08 * k);
            return Progress::Going(act);
        }
        if blow >= total + SETTLE_SECS {
            self.halt();
            return Progress::Finished;
        }
        // Затихает: хобот опускается, уши ложатся
        let on = if blow < total {
            1.0
        } else {
            1.0 - (blow - total) / SETTLE_SECS
        };
        let toot = (blow % TOOT_SECS) / TOOT_SECS;
        self.antic.head_pitch = RAISE * on;
        self.antic.neck_up = 4.0 * on;
        self.antic.pitch = 0.12 * on;
        self.antic.mouth = 0.6 * on;
        self.antic.wings = on;
        self.antic.flap = (blow * FLAP_HZ).fract();
        self.antic.wobble = 0.6 * on * (TAU * 2.0 * toot).sin();
        if blow < total {
            self.toot(&mut act, dt);
        }
        Progress::Going(act)
    }

    fn toot(&mut self, act: &mut Trumpet, dt: f32) {
        act.note -= dt;
        act.ring -= dt;
        if act.note <= 0.0 {
            act.note = NOTE_GAP;
            self.burst(Burst::Notes, self.head());
        }
        if act.ring <= 0.0 {
            act.ring = RING_GAP;
            self.burst(Burst::Rings, self.head());
        }
    }
}
