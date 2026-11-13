//! Разворот: такса упирается носом в край экрана и пробует развернуться, но длинное
//! тело не помещается — гармошкой, с круглыми глазами, то в одну сторону, то в другую.
//! Задумывается, складывается пополам и всё-таки уходит обратно.

use std::f32::consts::PI;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Логические px, рад, с.
const GATE: f32 = 30.0;
const ARRIVE: f32 = 24.0;
const WALK_SECS: f32 = 10.0;
const STOPPED: f32 = 20.0;
const TRIES: u8 = 2;
const TRY_SECS: f32 = 1.0;
/// Поворот попытки и сжатие тела в её разгаре.
const TRY_SPIN: f32 = 0.9;
const TRY_SQUASH: f32 = 0.82;
const PUZZLE_SECS: f32 = 1.0;
const FOLD_SECS: f32 = 0.8;
const FOLD_SQUASH: f32 = 0.7;
const LEAVE: f32 = 120.0;
const LEAVE_SECS: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Walk,
    Try(u8),
    Puzzle,
    Fold,
    Leave,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct UTurn {
    toward: Vec2,
    phase: Phase,
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_u_turn(&mut self) -> Option<UTurn> {
        let area = self.bounds.nearest(self.body.pos);
        let left = self.body.pos.x - area.min.x < area.max.x - self.body.pos.x;
        let toward = Vec2::new(if left { -1.0 } else { 1.0 }, 0.0);
        let edge = if left { area.min.x } else { area.max.x };
        let gate = Vec2::new(edge, self.body.pos.y) - toward * (GATE * self.ui);
        self.brain
            .push(Directive::MoveTo(area.clamp(gate, self.edge_margin())));
        Some(UTurn {
            toward,
            phase: Phase::Walk,
            t: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_u_turn(
        &mut self,
        mut act: UTurn,
        dt: f32,
    ) -> Progress<UTurn> {
        act.t += dt;
        let next = match act.phase {
            Phase::Walk => {
                let area = self.bounds.nearest(self.body.pos);
                let edge = if act.toward.x < 0.0 {
                    area.min.x
                } else {
                    area.max.x
                };
                let near = (edge - self.body.pos.x).abs()
                    <= (GATE + ARRIVE) * self.ui + self.edge_margin();
                if near {
                    self.halt();
                    Some(Phase::Try(0))
                } else if act.t > WALK_SECS {
                    return Progress::Dropped;
                } else {
                    None
                }
            }
            Phase::Try(n) => {
                let k = (act.t / TRY_SECS).min(1.0);
                let push = (PI * k).sin();
                // Попытки в разные стороны
                let side = if n % 2 == 0 { 1.0 } else { -1.0 };
                self.antic.spin = side * TRY_SPIN * push;
                self.antic.squash = Vec2::new(1.0 - (1.0 - TRY_SQUASH) * push, 1.0);
                self.antic.wobble = 1.2 * push * (PI * 14.0 * act.t).sin();
                if push > 0.8 {
                    self.antic.eyes = Eyes::Wide;
                }
                if k < 0.5 && (act.t + dt) / TRY_SECS >= 0.5 {
                    self.burst(Burst::Dust, self.head());
                }
                (act.t >= TRY_SECS).then_some(if n + 1 < TRIES {
                    Phase::Try(n + 1)
                } else {
                    Phase::Puzzle
                })
            }
            Phase::Puzzle => {
                self.antic.head_roll = 0.4;
                self.antic.neck_up = -1.0;
                (act.t >= PUZZLE_SECS).then(|| {
                    let area = self.bounds.nearest(self.body.pos);
                    let away = self.body.pos - act.toward * (LEAVE * self.ui);
                    self.brain
                        .push(Directive::MoveTo(area.clamp(away, self.edge_margin())));
                    Phase::Fold
                })
            }
            // Уходя, сперва складывается: длинное тело поворачивает гармошкой
            Phase::Fold => {
                let fold = (PI * (act.t / FOLD_SECS).min(1.0)).sin();
                self.antic.squash = Vec2::new(1.0 - (1.0 - FOLD_SQUASH) * fold, 1.0);
                self.antic.eyes = Eyes::Wide;
                (act.t >= FOLD_SECS).then_some(Phase::Leave)
            }
            Phase::Leave => {
                self.antic.wag = 1.0;
                if self.body.vel.length() < STOPPED * self.ui || act.t > LEAVE_SECS {
                    self.halt();
                    return Progress::Finished;
                }
                None
            }
        };
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }
}
