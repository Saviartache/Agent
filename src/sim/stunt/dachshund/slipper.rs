//! Тапок: откуда-то прилетает тапок, такса бежит за ним, хватает, треплет
//! и несёт к курсору (без курсора — на середину монитора), виляя хвостом.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, если не сказано «design», время — с.
const SPOT_DISTANCE: [f32; 2] = [140.0, 260.0];
const SPOT_MIN: f32 = 120.0;
const DROP_FROM: f32 = 70.0;
/// Кончик морды от точки под телом, design px.
const REACH: f32 = 30.0;
const ARRIVE: f32 = 16.0;
const GRAB_SECS: f32 = 0.4;
const SHAKE_SECS: f32 = 1.2;
const SHAKE_HZ: f32 = 6.0;
const PLACE_DISTANCE: f32 = 50.0;
const PROUD_SECS: f32 = 1.6;
const TIMEOUT: f32 = 25.0;
/// Тапок лежит столько после того, как его положили; запас жизни, пока несут.
const LIFE: f32 = 20.0;
const CARRY_LIFE: f32 = 40.0;
/// Тапок висит из пасти ниже кончика морды, design px.
const HANG: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Fetch,
    Grab(f32),
    Shake(f32),
    Bring,
    Proud(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Slipper {
    item: PropId,
    phase: Phase,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_slipper(&mut self) -> Option<Slipper> {
        let spot = self.spot_away(SPOT_DISTANCE, SPOT_MIN, ARRIVE)?;
        let angle = self.rng.range(0.0, TAU);
        let item = self.props.spawn(
            PropKind::Slipper,
            spot,
            DROP_FROM * self.ui,
            self.ui,
            CARRY_LIFE,
        );
        if let Some(slipper) = self.props.get_mut(item) {
            slipper.angle = angle;
        }
        let (_, stand) = self.reach_for(spot, REACH);
        self.brain.push(Directive::RunTo(stand));
        Some(Slipper {
            item,
            phase: Phase::Fetch,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_slipper(
        &mut self,
        mut act: Slipper,
        dt: f32,
    ) -> Progress<Slipper> {
        act.left -= dt;
        let Some(item) = self.props.get(act.item).copied() else {
            return Progress::Dropped;
        };
        if act.left <= 0.0 {
            self.props.remove(act.item);
            return Progress::Dropped;
        }
        act.phase = match act.phase {
            Phase::Fetch => {
                let (_, stand) = self.reach_for(item.pos, REACH);
                if item.resting() && self.body.pos.distance(stand) <= ARRIVE * self.ui {
                    self.halt();
                    Phase::Grab(0.0)
                } else {
                    Phase::Fetch
                }
            }
            Phase::Grab(t) => {
                // Морда к земле и обратно
                let dip = (std::f32::consts::PI * t / GRAB_SECS).sin();
                self.antic.neck_forward = 8.0 * dip;
                self.antic.neck_up = -12.0 * dip;
                self.antic.head_pitch = -0.9 * dip;
                if t >= GRAB_SECS * 0.5 {
                    self.carry_slipper(act.item);
                }
                if t + dt >= GRAB_SECS {
                    Phase::Shake(0.0)
                } else {
                    Phase::Grab(t + dt)
                }
            }
            Phase::Shake(t) => {
                let shake = (TAU * SHAKE_HZ * t).sin();
                self.antic.head_yaw = 0.8 * shake;
                self.antic.wobble = 1.5 * shake;
                self.antic.wag = 1.0;
                self.carry_slipper(act.item);
                if t + dt >= SHAKE_SECS {
                    self.burst(Burst::Dust, self.muzzle_ground());
                    Phase::Bring
                } else {
                    Phase::Shake(t + dt)
                }
            }
            Phase::Bring => {
                let area = self.bounds.nearest(self.body.pos);
                let goal = self.cursor.unwrap_or(area.center());
                self.brain.push(Directive::MoveTo(goal));
                self.antic.wag = 1.0;
                self.carry_slipper(act.item);
                if self.body.pos.distance(goal) <= PLACE_DISTANCE * self.ui {
                    self.halt();
                    if let Some(slipper) = self.props.get_mut(act.item) {
                        slipper.expire_in(LIFE);
                    }
                    self.burst(Burst::Hearts, self.head());
                    self.perform_quietly(self.character.actions().celebrate);
                    Phase::Proud(0.0)
                } else {
                    Phase::Bring
                }
            }
            Phase::Proud(t) if t < PROUD_SECS => {
                self.antic.wag = 1.0;
                Phase::Proud(t + dt)
            }
            Phase::Proud(_) => return Progress::Finished,
        };
        Progress::Going(act)
    }

    /// Тапок в пасти: под кончиком морды, на его высоте.
    fn carry_slipper(&mut self, id: PropId) {
        let (at, z) = (self.muzzle_ground(), self.muzzle_height());
        let z = (z - HANG * self.sprite).max(0.0);
        if let Some(slipper) = self.props.get_mut(id) {
            (slipper.pos, slipper.z, slipper.vel, slipper.vz) = (at, z, Vec2::ZERO, 0.0);
        }
    }
}
