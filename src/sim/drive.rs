//! Толчок тела: сценка ведёт тело сама — юз, скольжение пузом, отдача, полёт, — минуя
//! мозг и шаг. Тело упирается в край монитора так, чтобы нарисованное касалось края,
//! и сообщает об ударе (`Sim::bumped`); с `overhang` может зайти за край — прятки.

use super::Sim;
use crate::math::{Rect, Vec2};

/// Отступ точки на земле от края монитора, design px: бок, голова сверху, лапы снизу.
const INSET_SIDE: f32 = 22.0;
const INSET_TOP: f32 = 64.0;
const INSET_BOTTOM: f32 = 2.0;
/// Медленнее этого тело не разворачивается по скорости, px/с.
const HEADING_SPEED: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drive {
    /// Скорость, физические px/с.
    pub vel: Vec2,
    /// Затухание скорости, 1/с; 0 — без трения.
    pub friction: f32,
    /// Насколько тело может зайти за край монитора, физические px.
    pub overhang: f32,
}

impl Drive {
    pub fn new(vel: Vec2) -> Self {
        Self {
            vel,
            friction: 0.0,
            overhang: 0.0,
        }
    }

    pub fn friction(self, friction: f32) -> Self {
        Self { friction, ..self }
    }

    pub fn overhang(self, overhang: f32) -> Self {
        Self { overhang, ..self }
    }
}

impl Sim {
    /// Где может стоять точка на земле ведомого тела на мониторе `area`.
    pub(super) fn drive_room(&self, area: Rect, overhang: f32) -> Rect {
        let s = self.sprite;
        let o = overhang.max(0.0);
        Rect::new(
            area.min + Vec2::new(INSET_SIDE * s - o, INSET_TOP * s - o),
            area.max - Vec2::new(INSET_SIDE * s - o, INSET_BOTTOM * s - o),
        )
    }

    pub(super) fn drive_body(&mut self, dt: f32) {
        let Some(mut drive) = self.drive else {
            return;
        };
        let finite = |v: Vec2| v.x.is_finite() && v.y.is_finite();
        if !finite(drive.vel) {
            drive.vel = Vec2::ZERO;
        }
        drive.vel = drive.vel * (-drive.friction.max(0.0) * dt).exp();
        let room = self.drive_room(self.bounds.nearest(self.body.pos), drive.overhang);
        let next = self.body.pos + drive.vel * dt;
        let placed = room.clamp(next, 0.0);
        if placed != next {
            let normal = (placed - next).normalized();
            self.bumped = Some(normal);
            // Скорость в стену гасится, вдоль стены — остаётся
            let into = drive.vel.dot(normal).min(0.0);
            drive.vel = drive.vel - normal * into;
        }
        self.body.pos = placed;
        self.body.vel = drive.vel;
        if drive.vel.length() > HEADING_SPEED * self.ui {
            self.body.heading = drive.vel.normalized();
        }
        self.drive = Some(drive);
    }
}
