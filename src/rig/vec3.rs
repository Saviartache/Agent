//! Точка тела: `x`, `y` — земля мира, `z` — высота над землёй. На экране
//! точка рисуется в `(x, y − z)`.

use std::ops::{Add, Mul, Sub};

use crate::math::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn on_ground(ground: Vec2, z: f32) -> Self {
        Self::new(ground.x, ground.y, z)
    }

    pub fn ground(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Положение на экране.
    pub fn screen(self) -> Vec2 {
        Vec2::new(self.x, self.y - self.z)
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > f32::EPSILON {
            self * (1.0 / length)
        } else {
            Self::ZERO
        }
    }

    pub fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// Оси тела в мире: вперёд, вправо (по ходу) и вверх.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

impl Frame {
    /// `yaw` — угол хода на земле, `pitch` — подъём носа; `roll` крутит вокруг оси вперёд.
    pub fn new(yaw: f32, pitch: f32, roll: f32) -> Self {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();
        let forward = Vec3::new(cy * cp, sy * cp, sp);
        let right = Vec3::new(-sy, cy, 0.0);
        let up = Vec3::new(-cy * sp, -sy * sp, cp);
        Self {
            forward,
            right: right * cr + up * sr,
            up: up * cr - right * sr,
        }
    }

    /// Наклон вбок вокруг горизонтальной оси хода `yaw`, рад: плюс — макушкой вправо.
    pub fn rocked(self, yaw: f32, angle: f32) -> Self {
        if angle == 0.0 {
            return self;
        }
        let (sy, cy) = yaw.sin_cos();
        let (sa, ca) = angle.sin_cos();
        let side = Vec3::new(-sy, cy, 0.0);
        let rock = |v: Vec3| {
            let (a, b) = (v.dot(side), v.z);
            v + side * (a * ca + b * sa - a) + Vec3::new(0.0, 0.0, b * ca - a * sa - b)
        };
        Self {
            forward: rock(self.forward),
            right: rock(self.right),
            up: rock(self.up),
        }
    }

    pub fn point(&self, origin: Vec3, forward: f32, right: f32, up: f32) -> Vec3 {
        origin + self.forward * forward + self.right * right + self.up * up
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_orthonormal() {
        let frame = Frame::new(0.7, 0.3, 0.2);
        for (a, b) in [
            (frame.forward, frame.right),
            (frame.forward, frame.up),
            (frame.right, frame.up),
        ] {
            assert!(a.dot(b).abs() < 1e-5);
        }
        assert!((frame.up.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rocking_tilts_the_top_to_the_right_and_keeps_the_axes() {
        let frame = Frame::new(0.4, 1.2, 0.0).rocked(0.4, 0.3);
        let right = Frame::new(0.4, 0.0, 0.0).right;
        assert!(frame.forward.dot(right) > 0.25);
        for (a, b) in [
            (frame.forward, frame.right),
            (frame.forward, frame.up),
            (frame.right, frame.up),
        ] {
            assert!(a.dot(b).abs() < 1e-5);
        }
    }

    #[test]
    fn right_of_east_points_down_the_screen() {
        let frame = Frame::new(0.0, 0.0, 0.0);
        assert!(frame.right.y > 0.99);
        assert_eq!(Vec3::new(1.0, 5.0, 2.0).screen(), Vec2::new(1.0, 3.0));
    }
}
