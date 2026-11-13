//! Застряла: лезет за край, застревает попой наружу и вылетает пробкой.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, design px, с.
const APPROACH: f32 = 60.0;
const ARRIVE: f32 = 24.0;
const WALK_SECS: f32 = 10.0;
const CLIMB_SPEED: f32 = 140.0;
/// Насколько тело заходит за край.
const OVERHANG: f32 = 60.0;
const STUCK_SECS: f32 = 3.0;
const SCARED_BY: f32 = 100.0;
const WIGGLE_HZ: f32 = 9.0;
const KICK_HZ: f32 = 5.0;
const KICK_LIFT: f32 = 2.0;
const POP_SPEED: f32 = 400.0;
const POP_FRICTION: f32 = 4.0;
const POP_SPIN_SECS: f32 = 0.6;
const POP_DAZE: f32 = 0.5;
const STOPPED: f32 = 20.0;
const TIMEOUT: f32 = 12.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk,
    Climb,
    Wedged,
    Pop,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Stuck {
    /// Куда лезет: `(-1, 0)` — за левый край, `(1, 0)` — за правый.
    toward: Vec2,
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_stuck(&mut self) -> Option<Stuck> {
        let area = self.bounds.nearest(self.body.pos);
        let left = self.body.pos.x - area.min.x < area.max.x - self.body.pos.x;
        let toward = Vec2::new(if left { -1.0 } else { 1.0 }, 0.0);
        let edge = if left { area.min.x } else { area.max.x };
        let gate = Vec2::new(edge, self.body.pos.y) - toward * (APPROACH * self.ui);
        self.brain.push(Directive::MoveTo(area.clamp(gate, 0.0)));
        Some(Stuck {
            toward,
            phase: Phase::Walk,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_stuck(&mut self, mut act: Stuck, dt: f32) -> Progress<Stuck> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            return Progress::Dropped;
        }
        let overhang = OVERHANG * self.sprite;
        let mut next = None;
        match act.phase {
            Phase::Walk => {
                let area = self.bounds.nearest(self.body.pos);
                let edge = if act.toward.x < 0.0 {
                    area.min.x
                } else {
                    area.max.x
                };
                if (edge - self.body.pos.x).abs() <= (APPROACH + ARRIVE) * self.ui {
                    self.halt();
                    let vel = act.toward * (CLIMB_SPEED * self.ui);
                    self.drive = Some(Drive::new(vel).overhang(overhang));
                    next = Some(Phase::Climb);
                } else if act.t > WALK_SECS {
                    return Progress::Dropped;
                }
            }
            Phase::Climb => {
                if self.bumped.is_some() {
                    self.drive = Some(Drive::new(Vec2::ZERO).overhang(overhang));
                    next = Some(Phase::Wedged);
                }
            }
            Phase::Wedged => {
                let kick = (TAU * KICK_HZ * act.t).sin().abs();
                self.antic.wobble = 3.0 * (TAU * WIGGLE_HZ * act.t).sin();
                self.antic.lift = KICK_LIFT * kick;
                let scared = self
                    .cursor
                    .is_some_and(|c| c.distance(self.body.pos) < SCARED_BY * self.ui);
                if act.t >= STUCK_SECS || scared {
                    let vel = act.toward * (-POP_SPEED * self.ui);
                    self.drive = Some(Drive::new(vel).friction(POP_FRICTION).overhang(overhang));
                    self.burst(Burst::Stars, self.body.pos);
                    self.burst(Burst::Dust, self.body.pos);
                    self.cond.dazed = POP_DAZE;
                    next = Some(Phase::Pop);
                }
            }
            Phase::Pop => {
                self.antic.roll = TAU * (act.t / POP_SPIN_SECS).min(1.0);
                // Первый шаг скорость тела ещё старая
                if act.t > dt && self.body.vel.length() < STOPPED * self.ui {
                    self.drive = None;
                    self.halt();
                    return Progress::Finished;
                }
            }
        }
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }
}
