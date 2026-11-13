//! Хвостик: крутится на месте за хвостиком, потом шатается.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::sim::Sim;

/// Время — с, углы — рад, частоты — Гц.
const TURNS: f32 = 3.0;
const SPIN_SECS: f32 = 2.4;
const TOTAL_SECS: f32 = 4.0;
const HEAD_YAW: f32 = 1.2;
const WOBBLE: f32 = 2.0;
const WOBBLE_HZ: f32 = 3.0;
const DUST_GAP: f32 = 0.3;
const DAZED: f32 = 0.8;

#[derive(Debug)]
pub(in crate::sim) struct TailChase {
    spent: f32,
    dust: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_tail_chase(&mut self) -> Option<TailChase> {
        self.halt();
        Some(TailChase {
            spent: 0.0,
            dust: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_tail_chase(
        &mut self,
        mut act: TailChase,
        dt: f32,
    ) -> Progress<TailChase> {
        let before = act.spent;
        act.spent += dt;
        // Разгон: угол растёт квадратом времени
        let u = (act.spent / SPIN_SECS).min(1.0);
        self.antic.spin = TURNS * TAU * u * u;
        if act.spent < SPIN_SECS {
            self.antic.head_yaw = HEAD_YAW;
            self.antic.wobble = WOBBLE * u * (TAU * WOBBLE_HZ * act.spent).sin();
            act.dust -= dt;
            if act.dust <= 0.0 {
                act.dust = DUST_GAP;
                self.burst(Burst::Dust, self.body.pos);
            }
        } else if before < SPIN_SECS {
            self.cond.dazed = DAZED;
            self.burst(Burst::Stars, self.head());
        }
        if act.spent >= TOTAL_SECS {
            return Progress::Finished;
        }
        Progress::Going(act)
    }
}
