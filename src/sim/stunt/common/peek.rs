//! Прятки: уходит за ближний боковой край монитора целиком и выглядывает оттуда одной
//! головой. Курсор рядом с головой — прячется и выглядывает в другом месте края.
//! После трёх выглядываний выходит и радуется.

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::{Rect, Vec2};
use crate::sim::{Drive, Sim};

/// Время — с, расстояния — логические px, если не сказано «design».
const TIMEOUT: f32 = 20.0;
const PEEKS: u8 = 3;
/// Насколько тело может зайти за край, design px.
const OVERHANG: f32 = 110.0;
/// Глубина укрытия и выглядывания: на сколько тело за краем, design px.
const HIDDEN_DEPTH: f32 = 85.0;
const PEEK_DEPTH: f32 = 8.0;
const HIDE_SECS: f32 = 0.6;
const PEEK_SECS: f32 = 1.5;
/// Курсор ближе этого к голове — пора прятаться.
const SCARE_DISTANCE: f32 = 120.0;
/// Добежал до края, когда осталось столько, px.
const EDGE_REACH: f32 = 14.0;
const CHEER_SECS: f32 = 1.2;
/// Выглядывает в другом месте края: сдвиг по вертикали.
const SHIFT: [f32; 2] = [90.0, 260.0];
/// Скорость ухода в укрытие и выхода, доля скорости шага; выглядывает крадучись.
const HURRY: f32 = 1.8;
const SNEAK: f32 = 0.9;

#[derive(Debug)]
enum Phase {
    Run,
    /// Уходит в укрытие.
    Hide,
    /// Сидит за краем.
    Hidden(f32),
    /// Выезжает головой.
    Emerge,
    Look(f32),
    /// Выходит на экран.
    Return,
    Cheer(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Peek {
    /// −1 — левый край, 1 — правый.
    side: f32,
    phase: Phase,
    peeks: u8,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_peek(&mut self) -> Option<Peek> {
        let area = self.bounds.nearest(self.body.pos);
        let side = if self.body.pos.x - area.min.x < area.max.x - self.body.pos.x {
            -1.0
        } else {
            1.0
        };
        let at = Vec2::new(self.inner_x(area, side), self.body.pos.y);
        self.brain.push(Directive::RunTo(at));
        Some(Peek {
            side,
            phase: Phase::Run,
            peeks: 0,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_peek(&mut self, mut act: Peek, dt: f32) -> Progress<Peek> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let area = self.bounds.nearest(self.body.pos);
        let side = act.side;
        let [hidden, peek] = [HIDDEN_DEPTH, PEEK_DEPTH].map(|depth| self.edge_x(area, side, depth));
        match act.phase {
            Phase::Run => {
                if (self.body.pos.x - self.inner_x(area, side)).abs() <= EDGE_REACH * self.ui {
                    self.halt();
                    act.phase = Phase::Hide;
                }
            }
            Phase::Hide => {
                if self.slide_to(hidden, HURRY) {
                    self.shift_along(area);
                    act.phase = Phase::Hidden(HIDE_SECS);
                }
            }
            Phase::Hidden(left) => {
                self.slide_to(hidden, HURRY);
                act.phase = if left > dt {
                    Phase::Hidden(left - dt)
                } else {
                    Phase::Emerge
                };
            }
            Phase::Emerge => {
                if self.slide_to(peek, SNEAK) {
                    act.phase = Phase::Look(PEEK_SECS);
                }
            }
            Phase::Look(left) => {
                self.slide_to(peek, SNEAK);
                self.antic.neck_forward = 1.2;
                self.antic.head_yaw = 0.4 * (3.0 * self.clock).sin();
                let scared = self
                    .cursor
                    .is_some_and(|c| c.distance(self.head()) <= SCARE_DISTANCE * self.ui);
                if scared || left <= dt {
                    act.peeks += 1;
                    act.phase = if act.peeks >= PEEKS {
                        Phase::Return
                    } else {
                        Phase::Hide
                    };
                } else {
                    act.phase = Phase::Look(left - dt);
                }
            }
            Phase::Return => {
                let inside = self.inner_x(area, side);
                self.drive = Some(self.drive_toward(inside, HURRY));
                if (self.body.pos.x - inside) * side <= 3.0 * self.ui {
                    self.drive = None;
                    self.halt();
                    self.perform_quietly(self.character.actions().celebrate);
                    act.phase = Phase::Cheer(CHEER_SECS);
                }
            }
            Phase::Cheer(left) if left > dt => act.phase = Phase::Cheer(left - dt),
            Phase::Cheer(_) => return Progress::Finished,
        }
        Progress::Going(act)
    }

    /// Вертикаль на `depth` design px за краем монитора.
    fn edge_x(&self, area: Rect, side: f32, depth: f32) -> f32 {
        let edge = if side < 0.0 { area.min.x } else { area.max.x };
        edge + side * depth * self.sprite
    }

    /// Вертикаль, по которой ходит у края.
    fn inner_x(&self, area: Rect, side: f32) -> f32 {
        self.edge_x(area, side, 0.0) - side * self.edge_margin()
    }

    fn drive_toward(&self, x: f32, pace: f32) -> Drive {
        let gap = x - self.body.pos.x;
        let speed = self.character.motion().walk_speed * self.ui * pace;
        let vel = if gap.abs() < 2.0 * self.ui {
            0.0
        } else {
            speed * gap.signum()
        };
        Drive::new(Vec2::new(vel, 0.0)).overhang(OVERHANG * self.sprite)
    }

    /// Едет к вертикали `x`; `true` — приехал.
    fn slide_to(&mut self, x: f32, pace: f32) -> bool {
        self.drive = Some(self.drive_toward(x, pace));
        (self.body.pos.x - x).abs() < 3.0 * self.ui
    }

    /// Пока спрятан, переставляет тело вдоль края.
    fn shift_along(&mut self, area: Rect) {
        let sign = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let by = sign * self.rng.range(SHIFT[0], SHIFT[1]) * self.ui;
        let y = area.clamp(
            Vec2::new(self.body.pos.x, self.body.pos.y + by),
            self.edge_margin(),
        );
        self.body.pos.y = y.y;
    }
}
