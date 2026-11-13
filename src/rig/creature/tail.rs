//! Хвост на пружине: кончик отстаёт, когда тело трогается, тормозит и разворачивается.

use crate::math::Vec2;

/// Частота пружины, рад/с, и затухание (меньше 1 — перелёт).
const OMEGA: f32 = 9.0;
const DAMPING: f32 = 0.35;
/// Отставание кончика от ускорения тела, с².
const INERTIA: f32 = 0.01;
/// Поворот тела, рад/с, отбрасывает кончик вбок на столько design px.
const SWING: f32 = 1.5;
/// Дальше этого кончик не отстаёт, design px.
const MAX_LAG: f32 = 7.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Tail {
    /// Отставание кончика по земле, px.
    lag: Vec2,
    vel: Vec2,
}

impl Tail {
    pub fn lag(&self) -> Vec2 {
        self.lag
    }

    /// `turn` — поворот тела, рад/с; `right` — правый бок по земле.
    pub fn update(&mut self, dt: f32, accel: Vec2, turn: f32, right: Vec2, scale: f32) {
        let want = accel * -INERTIA - right * (turn * SWING * scale);
        let pull = (want - self.lag) * (OMEGA * OMEGA);
        let drag = self.vel * (2.0 * DAMPING * OMEGA);
        self.vel += (pull - drag) * dt;
        self.lag += self.vel * dt;
        let limit = MAX_LAG * scale;
        if !(self.lag.x.is_finite() && self.lag.y.is_finite()) {
            *self = Self::default();
        } else if self.lag.length() > limit {
            self.lag = self.lag.normalized() * limit;
        }
    }
}
