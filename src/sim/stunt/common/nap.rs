//! Сон: ложится, дремлет с «Zzz»; курсора не замечает. Иногда храпит с
//! подскоками и пылью. Выспавшись, встаёт сонным.

use std::f32::consts::PI;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const SECS: f32 = 20.0;
const LIE_SECS: f32 = 0.8;
const GET_UP_SECS: f32 = 0.5;
const ZZZ_GAP: f32 = 1.2;
const SNORE_CHANCE: f32 = 0.3;
const SNORE_GAP: f32 = 2.0;
const SNORE_SECS: f32 = 0.3;
const SNORE_HOP: f32 = 3.0;
const CROUCH: f32 = 5.5;

#[derive(Clone, Copy, Debug)]
enum Mode {
    Sleep,
    /// Выспался.
    Rise(f32),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Nap {
    mode: Mode,
    t: f32,
    /// Как низко лежит, `0…1`.
    down: f32,
    snores: bool,
    zzz: f32,
    snore: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_nap(&mut self) -> Option<Nap> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Nap {
            mode: Mode::Sleep,
            t: 0.0,
            down: 0.0,
            snores: self.rng.chance(SNORE_CHANCE),
            zzz: 0.0,
            snore: SNORE_GAP * 0.5,
        })
    }

    pub(in crate::sim::stunt) fn step_nap(&mut self, mut act: Nap, dt: f32) -> Progress<Nap> {
        act.t += dt;
        match act.mode {
            Mode::Sleep => {
                act.down = (act.down + dt / LIE_SECS).min(1.0);
                self.nap_doze(&mut act, dt);
                if act.t >= SECS {
                    act.mode = Mode::Rise(0.0);
                }
            }
            Mode::Rise(since) => {
                let since = since + dt;
                act.down = (act.down - dt / GET_UP_SECS).max(0.0);
                act.mode = Mode::Rise(since);
                if since >= GET_UP_SECS {
                    return self.nap_finish();
                }
            }
        }
        self.nap_lie_pose(act.down);
        Progress::Going(act)
    }

    /// Zzz и храп, пока спит.
    fn nap_doze(&mut self, act: &mut Nap, dt: f32) {
        act.zzz -= dt;
        if act.zzz <= 0.0 {
            act.zzz = ZZZ_GAP;
            self.burst(Burst::Zzz, self.head());
        }
        if !act.snores {
            return;
        }
        act.snore -= dt;
        if act.snore <= 0.0 {
            act.snore = SNORE_GAP;
            self.burst(Burst::Dust, self.body.pos);
        }
        let since = SNORE_GAP - act.snore;
        if since < SNORE_SECS {
            self.antic.lift = SNORE_HOP * (PI * since / SNORE_SECS).sin();
            self.antic.mouth = 0.4;
        }
    }

    pub(in crate::sim::stunt) fn nap_lie_pose(&mut self, down: f32) {
        let ease = down * down * (3.0 - 2.0 * down);
        self.antic.lids = self.antic.lids.max(ease);
        self.antic.tuck = ease;
        self.antic.crouch = CROUCH * ease;
        self.antic.neck_up -= 4.0 * ease;
    }

    fn nap_finish(&mut self) -> Progress<Nap> {
        self.cond.sleepy = 1.0;
        self.halt();
        Progress::Finished
    }
}
