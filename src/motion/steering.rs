//! Чистые функции рулёжки: каждая возвращает желаемую скорость.

use std::f32::consts::{PI, TAU};

use crate::math::{Rect, Vec2};
use crate::rng::Rng;

/// Шум направления блуждания, рад/с.
const WANDER_TURN_RATE: f32 = 5.0;
/// Скорость разворота от края, рад/с.
const EDGE_TURN_RATE: f32 = 4.0;
/// Запас торможения: останавливаемся заведомо раньше цели.
const BRAKE_FACTOR: f32 = 1.6;
/// Упреждение цели не дальше этого, с.
const MAX_LEAD_SECS: f32 = 1.5;

pub fn seek(pos: Vec2, target: Vec2, max_speed: f32) -> Vec2 {
    (target - pos).normalized() * max_speed
}

/// Скорость падает как корень от расстояния и не превышает того, что успеет затормозить.
pub fn arrive(
    pos: Vec2,
    target: Vec2,
    max_speed: f32,
    slow_radius: f32,
    deceleration: f32,
) -> Vec2 {
    let to = target - pos;
    let distance = to.length();
    let ramp = max_speed * (distance / slow_radius.max(f32::EPSILON)).sqrt();
    let brake = (BRAKE_FACTOR * deceleration * distance).sqrt();
    to.normalized() * ramp.min(brake).min(max_speed)
}

pub fn pursue(
    pos: Vec2,
    target: Vec2,
    target_vel: Vec2,
    max_speed: f32,
    deceleration: f32,
) -> Vec2 {
    let lead = (pos.distance(target) / max_speed.max(f32::EPSILON)).min(MAX_LEAD_SECS);
    arrive(
        pos,
        target + target_vel * lead,
        max_speed,
        max_speed * 0.5,
        deceleration,
    )
}

pub fn flee(pos: Vec2, from: Vec2, max_speed: f32) -> Vec2 {
    seek(from, pos, max_speed)
}

/// Угол дрейфует ограниченным шумом; у края плавно доворачивает внутрь (`inward`).
pub fn wander(angle: &mut f32, rng: &mut Rng, dt: f32, max_speed: f32, inward: Vec2) -> Vec2 {
    *angle += rng.range(-1.0, 1.0) * WANDER_TURN_RATE * dt;
    let push = inward.length() / max_speed.max(f32::EPSILON);
    if push > 0.0 {
        let diff = (inward.y.atan2(inward.x) - *angle + PI).rem_euclid(TAU) - PI;
        let limit = EDGE_TURN_RATE * push.min(1.0) * dt;
        *angle += diff.clamp(-limit, limit);
    }
    Vec2::from_angle(*angle) * max_speed
}

/// Мягкая сила внутрь на полосе `margin` у края; на самом краю вдвое сильнее `max_speed`.
pub fn avoid_bounds(pos: Vec2, hull: Rect, margin: f32, max_speed: f32) -> Vec2 {
    if margin <= 0.0 {
        return Vec2::ZERO;
    }
    let axis = |value: f32, min: f32, max: f32| {
        let low = ((min + margin - value) / margin).clamp(0.0, 1.0);
        let high = ((value - (max - margin)) / margin).clamp(0.0, 1.0);
        low - high
    };
    Vec2::new(
        axis(pos.x, hull.min.x, hull.max.x),
        axis(pos.y, hull.min.y, hull.max.y),
    ) * (max_speed * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrive_slows_near_target() {
        let target = Vec2::new(100.0, 0.0);
        let far = arrive(Vec2::ZERO, target, 70.0, 90.0, 640.0);
        let near = arrive(Vec2::new(98.0, 0.0), target, 70.0, 90.0, 640.0);
        assert_eq!(far.length(), 70.0);
        assert!(near.length() < 15.0);
    }

    #[test]
    fn avoid_bounds_pushes_inward_only_in_margin() {
        let hull = Rect::from_xywh(-100.0, -100.0, 200.0, 200.0);
        assert_eq!(avoid_bounds(Vec2::ZERO, hull, 20.0, 70.0), Vec2::ZERO);
        let push = avoid_bounds(Vec2::new(-95.0, 95.0), hull, 20.0, 70.0);
        assert!(push.x > 0.0 && push.y < 0.0);
    }

    #[test]
    fn flee_points_away() {
        let v = flee(Vec2::new(10.0, 0.0), Vec2::ZERO, 5.0);
        assert_eq!(v, Vec2::new(5.0, 0.0));
    }
}
