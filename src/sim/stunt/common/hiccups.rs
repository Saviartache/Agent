//! Икота: восемь иков с подскоком и пузырьками, последний — самый сильный.

use std::f32::consts::PI;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с, подскок — design px.
const COUNT: u32 = 8;
const GAP: f32 = 0.8;
const HOP: f32 = 6.0;
const LAST_HOP: f32 = 14.0;
const HOP_SECS: f32 = 0.25;
const WIDE_SECS: f32 = 0.2;
const REST_SECS: f32 = 0.4;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Hiccups {
    t: f32,
    done: u32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_hiccups(&mut self) -> Option<Hiccups> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Hiccups { t: 0.0, done: 0 })
    }

    pub(in crate::sim::stunt) fn step_hiccups(
        &mut self,
        mut act: Hiccups,
        dt: f32,
    ) -> Progress<Hiccups> {
        if act.t >= COUNT as f32 * GAP + REST_SECS {
            self.halt();
            return Progress::Finished;
        }
        if act.done < COUNT && act.t >= act.done as f32 * GAP {
            act.done += 1;
            self.burst(Burst::Bubbles, self.head());
        }
        act.t += dt;
        if let Some(index) = act.done.checked_sub(1) {
            let since = act.t - index as f32 * GAP;
            let height = if index + 1 == COUNT { LAST_HOP } else { HOP };
            self.antic.lift = height * (PI * since / HOP_SECS).sin().max(0.0);
            if since < WIDE_SECS {
                self.antic.eyes = Eyes::Wide;
            }
        }
        Progress::Going(act)
    }
}
