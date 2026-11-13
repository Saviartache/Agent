//! Сон с хоботом на земле: ложится боком к зрителю, кладёт хобот перед собой и сопит —
//! на каждом выдохе из-под кончика взлетает пыль. Выспавшись, встаёт сонным.

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Время — с, частоты — Гц.
const SECS: f32 = 14.0;
const LIE_SECS: f32 = 0.8;
const GET_UP_SECS: f32 = 0.5;
const ZZZ_GAP: f32 = 1.4;
/// Сопит: выдох раз в период.
const SNIFF_HZ: f32 = 0.5;
/// Кивок головы, укладывающий хобот на землю, рад.
const TRUNK_LAY: f32 = -0.8;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct TrunkNap {
    t: f32,
    /// Как низко лежит, 0…1.
    down: f32,
    zzz: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_trunk_nap(&mut self) -> Option<TrunkNap> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(TrunkNap {
            t: 0.0,
            down: 0.0,
            zzz: ZZZ_GAP * 0.5,
        })
    }

    pub(in crate::sim::stunt) fn step_trunk_nap(
        &mut self,
        mut act: TrunkNap,
        dt: f32,
    ) -> Progress<TrunkNap> {
        let was = act.t;
        act.t += dt;
        self.face_sideways();
        if act.t < SECS {
            act.down = (act.down + dt / LIE_SECS).min(1.0);
            self.sniff(&mut act, was, dt);
        } else {
            act.down = (act.down - dt / GET_UP_SECS).max(0.0);
            if act.t >= SECS + GET_UP_SECS {
                self.cond.sleepy = 1.0;
                self.drive = None;
                self.halt();
                return Progress::Finished;
            }
        }
        self.nap_lie_pose(act.down);
        self.antic.head_pitch = TRUNK_LAY * act.down;
        Progress::Going(act)
    }

    fn sniff(&mut self, act: &mut TrunkNap, was: f32, dt: f32) {
        act.zzz -= dt;
        if act.zzz <= 0.0 {
            act.zzz = ZZZ_GAP;
            self.burst(
                Burst::Zzz,
                self.body.pos - Vec2::new(0.0, 30.0 * self.sprite),
            );
        }
        if (was * SNIFF_HZ).fract() < 0.5 && (act.t * SNIFF_HZ).fract() >= 0.5 {
            self.burst(Burst::Dust, self.muzzle_ground());
        }
    }
}
