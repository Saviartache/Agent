//! Двумерная математика мира. Единица — физический пиксель виртуального экрана.

use std::ops::{Add, AddAssign, Mul, Neg, Sub};

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: Self) -> f32 {
        (other - self).length()
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Нулевой вектор остаётся нулевым.
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > f32::EPSILON {
            self * (1.0 / length)
        } else {
            Self::ZERO
        }
    }

    pub fn clamp_length(self, max: f32) -> Self {
        let length = self.length();
        if length > max && length > f32::EPSILON {
            self * (max / length)
        } else {
            self
        }
    }

    pub fn lerp(self, to: Self, t: f32) -> Self {
        self + (to - self) * t
    }

    pub fn from_angle(radians: f32) -> Self {
        Self::new(radians.cos(), radians.sin())
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

/// Полуоткрытый прямоугольник `[min, max)`; у мониторов `min` бывает отрицательным.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(Vec2::new(x, y), Vec2::new(x + width, y + height))
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn center(self) -> Vec2 {
        self.min.lerp(self.max, 0.5)
    }

    pub fn contains(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x < self.max.x
            && point.y >= self.min.y
            && point.y < self.max.y
    }

    /// Ближайшая точка внутри; отступ больше половины стороны сводит ось к центру.
    pub fn clamp(self, point: Vec2, margin: f32) -> Vec2 {
        let axis = |value: f32, min: f32, max: f32| {
            if max - min <= margin * 2.0 {
                (min + max) * 0.5
            } else {
                value.clamp(min + margin, max - margin)
            }
        };
        Vec2::new(
            axis(point.x, self.min.x, self.max.x),
            axis(point.y, self.min.y, self.max.y),
        )
    }

    pub fn union(self, other: Self) -> Self {
        Self::new(
            Vec2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            Vec2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        )
    }

    pub fn inflate(self, by: f32) -> Self {
        Self::new(self.min - Vec2::new(by, by), self.max + Vec2::new(by, by))
    }
}

/// Рабочие области мониторов: где персонажу можно находиться.
#[derive(Clone, Debug, PartialEq)]
pub struct Bounds {
    areas: Vec<Rect>,
}

impl Bounds {
    /// Пустой список недопустим: без мониторов персонажу негде быть.
    pub fn new(areas: Vec<Rect>) -> Option<Self> {
        (!areas.is_empty()).then_some(Self { areas })
    }

    pub fn areas(&self) -> &[Rect] {
        &self.areas
    }

    /// Охватывающий прямоугольник всех мониторов.
    pub fn hull(&self) -> Rect {
        self.areas[1..]
            .iter()
            .fold(self.areas[0], |hull, area| hull.union(*area))
    }

    pub fn contains(&self, point: Vec2) -> bool {
        self.areas.iter().any(|area| area.contains(point))
    }

    /// Область, ближайшая к точке (содержащая её — если такая есть).
    pub fn nearest(&self, point: Vec2) -> Rect {
        let gap = |area: &Rect| area.clamp(point, 0.0).distance(point);
        self.areas[1..].iter().fold(self.areas[0], |best, area| {
            if gap(area) < gap(&best) { *area } else { best }
        })
    }

    /// Возвращает точку на ближайший монитор — после отключения дисплея.
    pub fn clamp(&self, point: Vec2, margin: f32) -> Vec2 {
        self.nearest(point).clamp(point, margin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_pulls_point_onto_nearest_monitor() {
        let bounds = Bounds::new(vec![
            Rect::from_xywh(-1920.0, 0.0, 1920.0, 1080.0),
            Rect::from_xywh(0.0, 0.0, 2560.0, 1440.0),
        ])
        .unwrap();
        assert_eq!(
            bounds.clamp(Vec2::new(-3000.0, 500.0), 10.0),
            Vec2::new(-1910.0, 500.0)
        );
        assert_eq!(
            bounds.clamp(Vec2::new(100.0, 1400.0), 10.0),
            Vec2::new(100.0, 1400.0)
        );
        assert_eq!(bounds.hull(), Rect::from_xywh(-1920.0, 0.0, 4480.0, 1440.0));
    }

    #[test]
    fn clamp_centers_axis_narrower_than_margins() {
        let rect = Rect::from_xywh(0.0, 0.0, 10.0, 100.0);
        assert_eq!(
            rect.clamp(Vec2::new(-5.0, 200.0), 20.0),
            Vec2::new(5.0, 80.0)
        );
    }

    #[test]
    fn normalizing_zero_stays_zero() {
        assert_eq!(Vec2::ZERO.normalized(), Vec2::ZERO);
        assert_eq!(Vec2::new(3.0, 4.0).clamp_length(1.0).length(), 1.0);
    }
}
