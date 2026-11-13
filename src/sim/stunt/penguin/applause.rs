//! Аплодисменты: встаёт лицом к зрителю и хлопает ластами по бокам, подпрыгивая
//! на каждый хлопок; в конце — сердечки.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с; амплитуды — design px и радианы.
const SECS: f32 = 3.6;
const CLAP_HZ: f32 = 2.5;
const EASE_SECS: f32 = 0.3;
const HOP: f32 = 2.5;
const HEAD_PITCH: f32 = 0.25;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Applause {
    t: f32,
    claps: u32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_applause(&mut self) -> Option<Applause> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Applause { t: 0.0, claps: 0 })
    }

    pub(in crate::sim::stunt) fn step_applause(
        &mut self,
        mut act: Applause,
        dt: f32,
    ) -> Progress<Applause> {
        act.t += dt;
        if act.t >= SECS {
            self.burst(Burst::Hearts, self.head());
            self.halt();
            return Progress::Finished;
        }
        self.body.heading = Vec2::new(0.0, 1.0);
        let ease = (act.t / EASE_SECS).min((SECS - act.t) / EASE_SECS).min(1.0);
        let beat = CLAP_HZ * act.t;
        // Ласты врозь — и хлоп по бокам
        let open = 0.5 - 0.5 * (TAU * beat).cos();
        self.antic.wings = ease * open;
        self.antic.lift = ease * HOP * (1.0 - open);
        self.antic.head_pitch = ease * HEAD_PITCH * open;
        if beat as u32 > act.claps {
            act.claps += 1;
            self.burst(Burst::Sparks, self.head());
        }
        Progress::Going(act)
    }
}
