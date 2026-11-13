//! Тело персонажа и шаг интегрирования.

use super::steering;
use crate::math::{Bounds, Rect, Vec2};
use crate::rng::Rng;

/// Дольше за один шаг не интегрируем (пробуждение из сна и т. п.).
const MAX_DT: f32 = 0.25;
/// Медленнее этого цель считается достигнутой, px/с.
const STOP_SPEED: f32 = 8.0;
/// Ниже этой скорости направление движения не обновляется, px/с.
const HEADING_SPEED: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Единичный; последнее ненулевое направление движения.
    pub heading: Vec2,
}

impl Body {
    pub fn at(pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            heading: Vec2::new(1.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub max_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Steer {
    /// Плавно остановиться.
    Halt,
    Arrive {
        target: Vec2,
        slow_radius: f32,
        tolerance: f32,
    },
    /// Упреждает цель; вблизи подходит как `Arrive`.
    Pursue {
        target: Vec2,
        target_vel: Vec2,
        tolerance: f32,
    },
    Flee {
        from: Vec2,
    },
    /// Плавное блуждание, а не новая случайная точка каждую секунду.
    Wander,
}

/// Где можно быть и на какой полосе у края мягко отталкивать внутрь.
#[derive(Clone, Copy, Debug)]
pub struct Arena<'a> {
    pub bounds: &'a Bounds,
    pub margin: f32,
}

fn finite(v: Vec2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

#[derive(Clone, Debug, Default)]
pub struct Mover {
    wander_angle: f32,
}

impl Mover {
    pub fn new() -> Self {
        Self::default()
    }

    /// Один шаг. `true` — цель `Arrive`/`Pursue` достигнута (в допуске и почти стоим)
    /// либо `Halt` дошёл до покоя.
    pub fn step(
        &mut self,
        body: &mut Body,
        steer: &Steer,
        limits: &Limits,
        arena: &Arena<'_>,
        rng: &mut Rng,
        dt: f32,
    ) -> bool {
        let Arena { bounds, margin } = *arena;
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(MAX_DT)
        } else {
            0.0
        };
        let hull = bounds.hull();
        if !finite(body.pos) || !finite(body.vel) {
            *body = Body::at(hull.center());
        }
        let limits = Limits {
            max_speed: limits.max_speed.max(0.0),
            acceleration: limits.acceleration.max(0.0),
            deceleration: limits.deceleration.max(0.0),
        };
        let recovering = !bounds.contains(body.pos);
        let inside_hull = hull.clamp(body.pos, 0.0) == body.pos;
        let steer = if recovering {
            // Монитор пропал: возвращаемся шагом, а не телепортом
            Steer::Arrive {
                target: bounds.clamp(body.pos, margin.max(1.0)),
                slow_radius: margin.max(1.0),
                tolerance: 1.0,
            }
        } else {
            match *steer {
                Steer::Arrive { target, .. } | Steer::Pursue { target, .. } if !finite(target) => {
                    Steer::Halt
                }
                Steer::Arrive {
                    target,
                    slow_radius,
                    tolerance,
                } => Steer::Arrive {
                    target: hull.clamp(target, 0.0),
                    slow_radius,
                    tolerance,
                },
                other => other,
            }
        };

        if !matches!(steer, Steer::Wander) {
            self.wander_angle = body.heading.y.atan2(body.heading.x);
        }
        let inward = steering::avoid_bounds(body.pos, hull, margin, limits.max_speed);
        let desired = match steer {
            Steer::Halt => Vec2::ZERO,
            Steer::Arrive {
                target,
                slow_radius,
                ..
            } => steering::arrive(
                body.pos,
                target,
                limits.max_speed,
                slow_radius,
                limits.deceleration,
            ),
            Steer::Pursue {
                target, target_vel, ..
            } => steering::pursue(
                body.pos,
                target,
                if finite(target_vel) {
                    target_vel
                } else {
                    Vec2::ZERO
                },
                limits.max_speed,
                limits.deceleration,
            ),
            Steer::Flee { from } if finite(from) => {
                (steering::flee(body.pos, from, limits.max_speed) + inward)
                    .clamp_length(limits.max_speed)
            }
            Steer::Flee { .. } => Vec2::ZERO,
            Steer::Wander => {
                (steering::wander(&mut self.wander_angle, rng, dt, limits.max_speed, inward)
                    + inward)
                    .clamp_length(limits.max_speed)
            }
        };

        let rate = if desired.length() < body.vel.length() {
            limits.deceleration
        } else {
            limits.acceleration
        };
        body.vel += (desired - body.vel).clamp_length(rate * dt);
        let moved = body.vel * dt;
        let landed = match steer {
            Steer::Arrive { target, .. } => {
                let to = target - body.pos;
                // Шаг дотянулся до цели: встаём на неё, а не качаемся вокруг
                (moved.dot(to) > 0.0 && moved.length() >= to.length()).then_some(target)
            }
            _ => None,
        };
        match landed {
            Some(target) => {
                body.pos = target;
                body.vel = Vec2::ZERO;
            }
            None => body.pos += moved,
        }
        // Вне оболочки (монитор убрали) не телепортируем: возвращает рулёжка
        if inside_hull {
            Self::clamp_to_hull(body, hull);
        }
        if body.vel.length() > HEADING_SPEED {
            body.heading = body.vel.normalized();
        }

        if recovering {
            return false;
        }
        let speed = body.vel.length();
        match steer {
            Steer::Halt => speed <= f32::EPSILON,
            Steer::Arrive {
                target, tolerance, ..
            }
            | Steer::Pursue {
                target, tolerance, ..
            } => body.pos.distance(target) <= tolerance.max(0.0) && speed <= STOP_SPEED,
            _ => false,
        }
    }

    fn clamp_to_hull(body: &mut Body, hull: Rect) {
        let clamped = hull.clamp(body.pos, 0.0);
        if clamped.x != body.pos.x {
            body.vel.x = 0.0;
        }
        if clamped.y != body.pos.y {
            body.vel.y = 0.0;
        }
        body.pos = clamped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const LIMITS: Limits = Limits {
        max_speed: 70.0,
        acceleration: 420.0,
        deceleration: 640.0,
    };

    fn arena() -> Bounds {
        Bounds::new(vec![Rect::from_xywh(-1000.0, -500.0, 3000.0, 1500.0)]).unwrap()
    }

    fn arrive_at(target: Vec2) -> Steer {
        Steer::Arrive {
            target,
            slow_radius: 90.0,
            tolerance: 4.0,
        }
    }

    struct Run {
        body: Body,
        mover: Mover,
        rng: Rng,
        bounds: Bounds,
    }

    impl Run {
        fn new(pos: Vec2) -> Self {
            Self {
                body: Body::at(pos),
                mover: Mover::new(),
                rng: Rng::new(11),
                bounds: arena(),
            }
        }

        fn step(&mut self, steer: &Steer, limits: &Limits, dt: f32) -> bool {
            self.mover.step(
                &mut self.body,
                steer,
                limits,
                &Arena {
                    bounds: &self.bounds,
                    margin: 56.0,
                },
                &mut self.rng,
                dt,
            )
        }
    }

    #[test]
    fn velocity_change_and_speed_stay_within_limits() {
        let mut run = Run::new(Vec2::ZERO);
        let steer = arrive_at(Vec2::new(900.0, 300.0));
        for _ in 0..600 {
            let before = run.body.vel;
            run.step(&steer, &LIMITS, DT);
            let change = (run.body.vel - before).length();
            assert!(change <= LIMITS.deceleration * DT + 1e-3);
            assert!(run.body.vel.length() <= LIMITS.max_speed + 1e-3);
            assert!(run.body.vel.length() - before.length() <= LIMITS.acceleration * DT + 1e-3);
        }
    }

    #[test]
    fn arrive_stops_on_target_without_overshoot() {
        let mut run = Run::new(Vec2::ZERO);
        let target = Vec2::new(300.0, 0.0);
        let mut arrived = false;
        for _ in 0..1200 {
            arrived = run.step(&arrive_at(target), &LIMITS, DT);
            assert!(run.body.pos.x <= target.x + 1e-3);
            if arrived {
                break;
            }
        }
        assert!(arrived);
        assert!(run.body.pos.distance(target) <= 4.0);
        assert!(run.body.vel.length() <= STOP_SPEED);
    }

    #[test]
    fn halt_brakes_smoothly_to_rest() {
        let mut run = Run::new(Vec2::ZERO);
        run.body.vel = Vec2::new(70.0, 0.0);
        let mut previous = run.body.vel.length();
        let mut rested = false;
        for _ in 0..60 {
            rested = run.step(&Steer::Halt, &LIMITS, DT);
            let speed = run.body.vel.length();
            assert!(previous - speed <= LIMITS.deceleration * DT + 1e-3);
            previous = speed;
            if rested {
                break;
            }
        }
        assert!(rested);
    }

    #[test]
    fn switching_target_keeps_velocity_continuous() {
        let mut run = Run::new(Vec2::ZERO);
        for frame in 0..600 {
            let target = if frame < 150 {
                Vec2::new(800.0, 0.0)
            } else {
                Vec2::new(-400.0, 300.0)
            };
            let before = run.body.vel;
            run.step(&arrive_at(target), &LIMITS, DT);
            assert!((run.body.vel - before).length() <= LIMITS.deceleration * DT + 1e-3);
        }
    }

    #[test]
    fn step_size_does_not_change_travel() {
        let steer = arrive_at(Vec2::new(800.0, 0.0));
        let mut coarse = Run::new(Vec2::ZERO);
        let mut fine = Run::new(Vec2::ZERO);
        for _ in 0..60 {
            coarse.step(&steer, &LIMITS, 0.1);
            for _ in 0..10 {
                fine.step(&steer, &LIMITS, 0.01);
            }
        }
        assert!(coarse.body.pos.distance(fine.body.pos) < 12.0);
    }

    #[test]
    fn degenerate_dt_is_harmless() {
        let steer = arrive_at(Vec2::new(500.0, 0.0));
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, 1e6] {
            let mut run = Run::new(Vec2::ZERO);
            run.body.vel = Vec2::new(10.0, 0.0);
            for steer in [steer, Steer::Wander, Steer::Halt] {
                run.step(&steer, &LIMITS, dt);
                assert!(run.body.pos.x.is_finite() && run.body.vel.x.is_finite());
                assert!(run.body.pos.y.is_finite() && run.body.vel.y.is_finite());
            }
        }
        let mut run = Run::new(Vec2::ZERO);
        run.step(&steer, &LIMITS, 0.0);
        run.step(&steer, &LIMITS, f32::NAN);
        assert_eq!(run.body, Body::at(Vec2::ZERO));
    }

    #[test]
    fn huge_dt_lands_on_target() {
        let mut run = Run::new(Vec2::ZERO);
        let target = Vec2::new(50.0, 0.0);
        let mut arrived = false;
        for _ in 0..200 {
            arrived = run.step(&arrive_at(target), &LIMITS, 1e6);
            if arrived {
                break;
            }
        }
        assert!(arrived);
        assert_eq!(run.body.pos, target);
    }

    #[test]
    fn wander_stays_inside_hull_with_negative_coordinates() {
        let mut run = Run::new(Vec2::new(-900.0, -400.0));
        run.bounds = Bounds::new(vec![Rect::from_xywh(-1000.0, -500.0, 600.0, 400.0)]).unwrap();
        let hull = run.bounds.hull();
        for _ in 0..60 * 600 {
            run.step(&Steer::Wander, &LIMITS, DT);
            assert!(hull.contains(run.body.pos) || run.body.pos == hull.clamp(run.body.pos, 0.0));
            assert!(run.body.vel.length() <= LIMITS.max_speed + 1e-3);
        }
    }

    #[test]
    fn flee_keeps_inside_hull() {
        let mut run = Run::new(Vec2::new(-950.0, 0.0));
        let hull = run.bounds.hull();
        for _ in 0..600 {
            run.step(
                &Steer::Flee {
                    from: Vec2::new(-1000.0, 0.0),
                },
                &LIMITS,
                DT,
            );
            assert!(run.body.pos.x >= hull.min.x && run.body.pos.x <= hull.max.x);
        }
    }

    #[test]
    fn returns_on_foot_after_monitor_removal() {
        let mut run = Run::new(Vec2::new(1500.0, 100.0));
        let mut previous = run.body.pos;
        run.bounds = Bounds::new(vec![Rect::from_xywh(0.0, 0.0, 1000.0, 800.0)]).unwrap();
        let mut frames = 0;
        while !run.bounds.contains(run.body.pos) && frames < 6000 {
            run.step(&Steer::Wander, &LIMITS, DT);
            assert!(run.body.pos.distance(previous) <= LIMITS.max_speed * DT + 1e-3);
            previous = run.body.pos;
            frames += 1;
        }
        assert!(run.bounds.contains(run.body.pos));
        assert!(frames > 60);
    }

    #[test]
    fn wander_heading_changes_gradually() {
        let mut run = Run::new(Vec2::new(500.0, 250.0));
        let angle = |v: Vec2| v.y.atan2(v.x);
        let mut previous = angle(run.body.heading);
        for frame in 0..60 * 20 {
            run.step(&Steer::Wander, &LIMITS, DT);
            let current = angle(run.body.heading);
            let turn = (current - previous + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            // Не больше 6 рад/с
            assert!(turn.abs() <= 6.0 * DT + 1e-3, "frame {frame}: {turn}");
            previous = current;
        }
    }

    #[test]
    fn pursue_reaches_stationary_target() {
        let mut run = Run::new(Vec2::ZERO);
        let steer = Steer::Pursue {
            target: Vec2::new(200.0, 0.0),
            target_vel: Vec2::ZERO,
            tolerance: 4.0,
        };
        let reached = (0..1800).any(|_| run.step(&steer, &LIMITS, DT));
        assert!(reached);
    }
}
