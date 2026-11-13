//! Чистка перьев: клюв к боку, клевки в перья по очереди с обеих сторон;
//! перо выпадает из-под клюва.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::math::Vec2;
use crate::sim::Sim;

/// Время — с, углы — рад, расстояния — design px.
const PECK_SECS: f32 = 0.7;
const PECKS_PER_SIDE: u8 = 3;
const SIDES: u8 = 2;
/// Голова отведена назад и повёрнута к боку так, что кончик клюва лежит на перьях.
const HEAD_YAW: f32 = 1.4;
const NECK_FORWARD: f32 = -12.0;
const NECK_UP: f32 = -8.0;
const HEAD_PITCH: f32 = -0.6;
const PECK_DIP: f32 = 0.3;

#[derive(Debug)]
pub(in crate::sim) struct Preen {
    /// Первый бок: −1 или 1.
    first: f32,
    t: f32,
    /// Клевков сделано.
    pecked: u8,
    /// С какого клевка роняет перья.
    sheds: [u8; 2],
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
        })
    }

    pub(in crate::sim::stunt) fn step_preen(&mut self, mut act: Preen, dt: f32) -> Progress<Preen> {
        act.t += dt;
        let total = PECKS_PER_SIDE * SIDES;
        let index = (act.t / PECK_SECS) as u8;
        if index >= total {
            return Progress::Finished;
        }
        let side = if (index / PECKS_PER_SIDE).is_multiple_of(2) {
            act.first
        } else {
            -act.first
        };
        let local = act.t - f32::from(index) * PECK_SECS;
        let dip = (TAU * local / PECK_SECS).sin().max(0.0);
        let a = &mut self.antic;
        a.head_yaw = HEAD_YAW * side;
        a.neck_forward = NECK_FORWARD;
        a.neck_up = NECK_UP;
        a.head_pitch = HEAD_PITCH - PECK_DIP * dip;
        // Перо — из-под клюва, когда он в перьях
        if index == act.pecked && local >= PECK_SECS * 0.25 {
            act.pecked += 1;
            if act.sheds.contains(&index) {
                self.shed_feather(Vec2::ZERO);
            }
        }
        Progress::Going(act)
    }
}
