//! Гогот: шея вверх, клюв вверх, кольца гогота от головы; бывает, отдача от последнего
//! роняет на хвост, и он обиженно гогочет потише.

use super::super::Progress;
use crate::behavior::Facing;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с, скорость — логические px/с.
const HONK_GAP: f32 = 0.8;
/// Клюв открыт столько от начала гогота.
const HONK_OPEN: f32 = 0.35;
const RING_GAP: f32 = 0.15;
const RECOIL_CHANCE: f32 = 0.3;
const RECOIL_SPEED: f32 = 200.0;
const RECOIL_FRICTION: f32 = 6.0;
const RECOIL_SECS: f32 = 0.35;
const SIT_SECS: f32 = 1.0;
const SULK_SECS: f32 = 0.9;
const TAIL_PITCH: f32 = 0.8;
const NECK_UP: f32 = 10.0;
/// Огромный гогот: шея и клюв ещё выше, колец больше.
const HUGE_NECK_UP: f32 = 13.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Honk,
    Recoil,
    Sit,
    Sulk,
}

#[derive(Debug)]
pub(in crate::sim) struct Honk {
    phase: Phase,
    t: f32,
    honks: u8,
    /// Начато гоготов.
    started: u8,
    recoil: bool,
    rings: u8,
    ring_in: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_honk(&mut self) -> Option<Honk> {
        self.halt();
        Some(Honk {
            phase: Phase::Honk,
            t: 0.0,
            honks: 1 + self.pick(3) as u8,
            started: 0,
            recoil: self.rng.chance(RECOIL_CHANCE),
            rings: 0,
            ring_in: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_honk(&mut self, mut act: Honk, dt: f32) -> Progress<Honk> {
        act.t += dt;
        match act.phase {
            Phase::Honk => self.honk(&mut act, dt),
            Phase::Recoil => {
                let k = (act.t / RECOIL_SECS).min(1.0);
                self.antic.pitch = TAIL_PITCH * k;
                self.antic.neck_up = HUGE_NECK_UP * (1.0 - k);
                self.antic.mouth = if act.t < 0.2 { 1.0 } else { 0.0 };
                if act.t >= RECOIL_SECS {
                    enter(&mut act, Phase::Sit);
                }
            }
            Phase::Sit => {
                self.antic.pitch = TAIL_PITCH;
                self.antic.crouch = 0.3;
                if act.t >= SIT_SECS {
                    self.ring();
                    enter(&mut act, Phase::Sulk);
                }
            }
            Phase::Sulk => {
                self.antic.pitch = TAIL_PITCH * (1.0 - act.t / 0.3).max(0.0);
                self.antic.mouth = 0.4;
                self.antic.lids = 0.4;
                if act.t >= SULK_SECS {
                    return self.end_honk();
                }
            }
        }
        if act.phase == Phase::Honk && act.t >= f32::from(act.honks) * HONK_GAP {
            if !act.recoil {
                return self.end_honk();
            }
            let back = match self.facing {
                Facing::Left => 1.0,
                Facing::Right => -1.0,
            };
            let vel = Vec2::new(back * RECOIL_SPEED * self.ui, 0.0);
            self.drive = Some(Drive::new(vel).friction(RECOIL_FRICTION));
            enter(&mut act, Phase::Recoil);
        }
        Progress::Going(act)
    }

    fn honk(&mut self, act: &mut Honk, dt: f32) {
        if act.started < act.honks && act.t >= f32::from(act.started) * HONK_GAP {
            act.started += 1;
            let huge = act.recoil && act.started == act.honks;
            act.rings = if huge { 4 } else { 2 + self.pick(2) as u8 };
            act.ring_in = 0.0;
        }
        act.ring_in -= dt;
        if act.rings > 0 && act.ring_in <= 0.0 {
            act.rings -= 1;
            act.ring_in = RING_GAP;
            self.ring();
        }
        let huge = act.recoil && act.started == act.honks;
        let local = act.t - f32::from(act.started.saturating_sub(1)) * HONK_GAP;
        self.antic.neck_up = if huge { HUGE_NECK_UP } else { NECK_UP };
        self.antic.head_pitch = if huge { 0.8 } else { 0.5 };
        self.antic.mouth = if local < HONK_OPEN { 1.0 } else { 0.0 };
    }

    fn ring(&mut self) {
        self.burst(Burst::Rings, self.head());
    }

    fn end_honk(&mut self) -> Progress<Honk> {
        self.drive = None;
        self.halt();
        Progress::Finished
    }
}

fn enter(act: &mut Honk, phase: Phase) {
    act.phase = phase;
    act.t = 0.0;
}
