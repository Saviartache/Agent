//! Оступается у нижнего края монитора, кувыркается за него, пропадает из виду,
//! медленно вылезает обратно и отряхивается.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::{Drive, Sim};

/// Расстояния — логические px, скорости — px/с, время — с.
const TIMEOUT: f32 = 12.0;
/// Насколько тело может зайти за нижний край, design px: пропадает целиком.
const OVERHANG: f32 = 110.0;
const EDGE_REACH: f32 = 14.0;
const STUMBLE_SECS: f32 = 0.6;
const FALL_SPEED: f32 = 260.0;
/// Полный кувырок за падение.
const TUMBLES: f32 = 1.0;
const GONE_SECS: f32 = 1.0;
const CLIMB_SPEED: f32 = 70.0;
const SHAKE_SECS: f32 = 1.0;

#[derive(Debug)]
enum Phase {
    Walk,
    Stumble(f32),
    Fall,
    Gone(f32),
    Climb,
    Shake(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct EdgeTumble {
    phase: Phase,
    /// Сколько падает: доля кувырка.
    spun: f32,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_edge_tumble(&mut self) -> Option<EdgeTumble> {
        self.brain.push(Directive::RunTo(self.floor_below()));
        Some(EdgeTumble {
            phase: Phase::Walk,
            spun: 0.0,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_edge_tumble(
        &mut self,
        mut act: EdgeTumble,
        dt: f32,
    ) -> Progress<EdgeTumble> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let hole = OVERHANG * self.sprite;
        match act.phase {
            Phase::Walk => {
                let floor = self.floor_below();
                self.brain.push(Directive::RunTo(floor));
                if self.body.pos.distance(floor) <= EDGE_REACH * self.ui {
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    act.phase = Phase::Stumble(STUMBLE_SECS);
                }
            }
            Phase::Stumble(left) => {
                self.antic.eyes = Eyes::Wide;
                self.antic.wobble = 4.0 * (TAU * 3.0 * left).sin();
                self.antic.pitch = 0.3 * (STUMBLE_SECS - left) / STUMBLE_SECS;
                act.phase = if left > dt {
                    Phase::Stumble(left - dt)
                } else {
                    Phase::Fall
                };
            }
            Phase::Fall => {
                self.drive = Some(Drive::new(Vec2::new(0.0, FALL_SPEED * self.ui)).overhang(hole));
                act.spun = (act.spun + dt * 2.0).min(TUMBLES);
                self.antic.eyes = Eyes::Wide;
                self.antic.roll = TAU * act.spun;
                if self.bumped.is_some() {
                    self.burst(Burst::Dust, self.body.pos);
                    act.phase = Phase::Gone(GONE_SECS);
                }
            }
            Phase::Gone(left) => {
                self.drive = Some(Drive::new(Vec2::ZERO).overhang(hole));
                act.phase = if left > dt {
                    Phase::Gone(left - dt)
                } else {
                    Phase::Climb
                };
            }
            Phase::Climb => {
                self.drive =
                    Some(Drive::new(Vec2::new(0.0, -CLIMB_SPEED * self.ui)).overhang(hole));
                self.antic.neck_forward = 0.7;
                self.antic.crouch = 0.4;
                if self.body.pos.y <= self.floor_below().y {
                    // Отряхивается на месте
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    self.burst(Burst::Dust, self.body.pos);
                    act.phase = Phase::Shake(SHAKE_SECS);
                }
            }
            Phase::Shake(left) => {
                self.antic.wobble = 3.5 * (TAU * 4.0 * left).sin();
                if left <= dt {
                    return Progress::Finished;
                }
                act.phase = Phase::Shake(left - dt);
            }
        }
        Progress::Going(act)
    }

    /// Где ходит у нижнего края монитора.
    fn floor_below(&self) -> Vec2 {
        let area = self.bounds.nearest(self.body.pos);
        Vec2::new(self.body.pos.x, area.max.y - self.edge_margin())
    }
}
