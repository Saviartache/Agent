//! Кверху пузом: переворачивается ножками вверх и дрыгает ими.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::sim::Sim;

/// Время — с, углы — рад, частоты — Гц.
const SIDE_SECS: f32 = 0.4;
const BACK_SECS: f32 = 0.4;
const KICK_UNTIL: f32 = 4.0;
const KICK_HZ: f32 = 5.0;
const WOBBLE: f32 = 2.0;
const LIDS: f32 = 0.5;
const HEARTS_AT: f32 = 1.2;
const FLIP_BACK_SECS: f32 = 0.6;
const SHAKE_SECS: f32 = 0.4;
const SHAKE_HZ: f32 = 8.0;

#[derive(Debug)]
pub(in crate::sim) struct BellyUp {
    spent: f32,
}

fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_belly_up(&mut self) -> Option<BellyUp> {
        self.halt();
        Some(BellyUp { spent: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_belly_up(
        &mut self,
        mut act: BellyUp,
        dt: f32,
    ) -> Progress<BellyUp> {
        let before = act.spent;
        act.spent += dt;
        let t = act.spent;
        let flipped = SIDE_SECS + BACK_SECS;
        let back_at = KICK_UNTIL;
        let up_at = back_at + FLIP_BACK_SECS;
        let end = up_at + SHAKE_SECS;
        if t < flipped {
            self.antic.roll =
                PI * 0.5 * smooth(t / SIDE_SECS) + PI * 0.5 * smooth((t - SIDE_SECS) / BACK_SECS);
        } else if t < back_at {
            self.antic.roll = PI;
            self.antic.lids = LIDS;
            self.antic.wobble = WOBBLE * (TAU * KICK_HZ * 1.7 * t).sin();
            self.antic.tuck = 0.5 + 0.5 * (TAU * KICK_HZ * t).sin();
            if before < HEARTS_AT && t >= HEARTS_AT {
                self.burst(Burst::Hearts, self.head());
            }
        } else if t < up_at {
            self.antic.roll = PI * (1.0 - smooth((t - back_at) / FLIP_BACK_SECS));
        } else {
            if before < up_at {
                self.burst(Burst::Dust, self.body.pos);
            }
            self.antic.wobble = WOBBLE * (TAU * SHAKE_HZ * (t - up_at)).sin();
        }
        if t >= end {
            return Progress::Finished;
        }
        Progress::Going(act)
    }
}
