//! Чистка перьев: голова к боку, клевки в перья по очереди с обеих сторон, выпадает перо;
//! бывает, перестарался и вычистил проплешину.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Время — с.
const PECK_SECS: f32 = 0.7;
const PECKS_PER_SIDE: u8 = 3;
const SIDES: u8 = 2;
const HEAD_YAW: f32 = 1.6;
const PECK_DIP: f32 = 0.3;
const BALD_CHANCE: f32 = 0.3;
const INSPECT_SECS: f32 = 1.2;

#[derive(Debug)]
pub(in crate::sim) struct Preen {
    /// Первый бок: −1 или 1.
    first: f32,
    t: f32,
    /// Клевков сделано.
    pecked: u8,
    /// С какого клевка роняет перья.
    sheds: [u8; 2],
    overdo: bool,
    inspect: Option<f32>,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_preen(&mut self) -> Option<Preen> {
        self.halt();
        let first = if self.rng.chance(0.5) { -1.0 } else { 1.0 };
        let sheds = [1 + self.pick(2) as u8, 3 + self.pick(3) as u8];
        let drops = 1 + self.pick(2);
        Some(Preen {
            first,
            t: 0.0,
            pecked: 0,
            sheds: if drops == 1 {
                [sheds[0], u8::MAX]
            } else {
                sheds
            },
            overdo: self.rng.chance(BALD_CHANCE),
            inspect: None,
        })
    }

    pub(in crate::sim::stunt) fn step_preen(&mut self, mut act: Preen, dt: f32) -> Progress<Preen> {
        if let Some(t) = act.inspect {
            return self.inspect(act, t + dt);
        }
        act.t += dt;
        let total = PECKS_PER_SIDE * SIDES;
        let index = (act.t / PECK_SECS) as u8;
        if index >= total {
            if !act.overdo {
                return Progress::Finished;
            }
            self.cond.bald = 1.0;
            act.inspect = Some(0.0);
            return Progress::Going(act);
        }
        let side = if (index / PECKS_PER_SIDE).is_multiple_of(2) {
            act.first
        } else {
            -act.first
        };
        let local = act.t - f32::from(index) * PECK_SECS;
        if index == act.pecked {
            act.pecked += 1;
            if act.sheds.contains(&index) {
                self.shed_feather(self.body.pos, Vec2::ZERO);
            }
        }
        let a = &mut self.antic;
        a.head_yaw = HEAD_YAW * side;
        a.neck_side = 3.0 * side;
        a.neck_forward = -2.0;
        a.head_pitch = -0.5 - PECK_DIP * (TAU * local / PECK_SECS).sin().max(0.0);
        Progress::Going(act)
    }

    fn inspect(&mut self, mut act: Preen, t: f32) -> Progress<Preen> {
        if t >= INSPECT_SECS {
            return Progress::Finished;
        }
        act.inspect = Some(t);
        let a = &mut self.antic;
        a.head_yaw = HEAD_YAW * act.first;
        a.neck_side = 3.0 * act.first;
        a.neck_forward = -2.0;
        a.head_pitch = -0.4;
        a.eyes = Eyes::Wide;
        Progress::Going(act)
    }
}
