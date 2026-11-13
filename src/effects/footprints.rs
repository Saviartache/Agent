//! Временные следы за персонажем: не зависят от текущей анимации.

use std::collections::VecDeque;

use super::{bounding, sane_dt};
use crate::animation::tween::Easing;
use crate::math::{Rect, Vec2};

/// Доля жизни, в течение которой след остаётся чётким.
const SOLID: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub pos: Vec2,
    /// Единичный.
    pub heading: Vec2,
    pub left: bool,
    /// От 1 до 0: таяние по возрасту на вес отпечатка.
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug)]
struct Print {
    pos: Vec2,
    heading: Vec2,
    left: bool,
    /// Насколько отпечаток жирный, 0…1: грязь на копытах стирается шаг за шагом.
    weight: f32,
    age: f32,
}

#[derive(Clone, Debug)]
pub struct Footprints {
    prints: VecDeque<Print>,
    capacity: usize,
    lifetime: f32,
    spacing: f32,
    /// Точка на тропе, где оставлен последний след; `None` — тропа прервана.
    anchor: Option<Vec2>,
    next_left: bool,
}

impl Footprints {
    pub fn new(capacity: usize, lifetime: f32, spacing: f32) -> Self {
        let capacity = capacity.max(1);
        Self {
            prints: VecDeque::with_capacity(capacity),
            capacity,
            lifetime: lifetime.max(f32::EPSILON),
            // Защита от бесконечного цикла в track
            spacing: spacing.max(1.0),
            anchor: None,
            next_left: true,
        }
    }

    pub fn track(&mut self, pos: Vec2, heading: Vec2, half_gauge: f32, grounded: bool) {
        if !grounded {
            self.anchor = None;
            return;
        }
        if !(pos.x.is_finite() && pos.y.is_finite()) {
            return;
        }
        let Some(mut anchor) = self.anchor else {
            self.anchor = Some(pos);
            return;
        };
        let delta = pos - anchor;
        let distance = delta.length();
        if distance < self.spacing {
            return;
        }
        let direction = delta * (1.0 / distance);
        let heading = match heading.normalized() {
            h if h == Vec2::ZERO => direction,
            h => h,
        };
        let total = (distance / self.spacing) as usize;
        // Следы сверх ёмкости всё равно вытеснились бы
        let skip = total.saturating_sub(self.capacity);
        anchor += direction * (self.spacing * skip as f32);
        for _ in skip..total {
            anchor += direction * self.spacing;
            let side = if self.next_left {
                Vec2::new(heading.y, -heading.x)
            } else {
                Vec2::new(-heading.y, heading.x)
            };
            self.push(anchor + side * half_gauge, heading, self.next_left, 1.0);
            self.next_left = !self.next_left;
        }
        self.anchor = Some(anchor);
    }

    /// След от шага тела, которое само знает, куда ставит лапу. Ближе `clear` к
    /// лежащему следу новый не ложится: у четвероногого задняя нога встаёт в след
    /// передней, и отпечатки слиплись бы в один. `weight` — жирность отпечатка, 0…1.
    pub fn stamp(&mut self, pos: Vec2, heading: Vec2, left: bool, clear: f32, weight: f32) {
        if !(pos.x.is_finite() && pos.y.is_finite()) {
            return;
        }
        let heading = heading.normalized();
        let nearest = self
            .prints
            .iter()
            .map(|print| print.pos)
            .min_by(|a, b| a.distance(pos).total_cmp(&b.distance(pos)));
        let pos = match nearest {
            Some(near) if near.distance(pos) < clear => {
                let away = match (pos - near).normalized() {
                    v if v == Vec2::ZERO => heading * -1.0,
                    v => v,
                };
                near + away * clear
            }
            _ => pos,
        };
        self.push(pos, heading, left, weight);
    }

    fn push(&mut self, pos: Vec2, heading: Vec2, left: bool, weight: f32) {
        if self.prints.len() == self.capacity {
            self.prints.pop_front();
        }
        self.prints.push_back(Print {
            pos,
            heading,
            left,
            weight: weight.clamp(0.0, 1.0),
            age: 0.0,
        });
    }

    pub fn update(&mut self, dt: f32) {
        let dt = sane_dt(dt);
        for print in &mut self.prints {
            print.age += dt;
        }
        // Старшие следы всегда первые
        while self.prints.front().is_some_and(|p| p.age >= self.lifetime) {
            self.prints.pop_front();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Footprint> + '_ {
        self.prints.iter().map(|print| Footprint {
            pos: print.pos,
            heading: print.heading,
            left: print.left,
            opacity: self.opacity(print.age) * print.weight,
        })
    }

    pub fn bounds(&self, radius: f32) -> Option<Rect> {
        bounding(self.prints.iter().map(|print| (print.pos, radius)))
    }

    pub fn is_empty(&self) -> bool {
        self.prints.is_empty()
    }

    pub fn clear(&mut self) {
        self.prints.clear();
        self.anchor = None;
    }

    fn opacity(&self, age: f32) -> f32 {
        let melt = (age / self.lifetime - SOLID) / (1.0 - SOLID);
        1.0 - Easing::InOutCubic.apply(melt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIGHT: Vec2 = Vec2::new(1.0, 0.0);

    fn walk(prints: &mut Footprints, from: f32, to: f32, step: f32) {
        let mut x = from;
        while x < to {
            x += step;
            prints.track(Vec2::new(x, 0.0), RIGHT, 4.0, true);
        }
    }

    #[test]
    fn spacing_decides_count() {
        let mut prints = Footprints::new(64, 10.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 4.0, true);
        walk(&mut prints, 0.0, 100.0, 5.0);
        assert_eq!(prints.iter().count(), 10);
    }

    #[test]
    fn prints_alternate_and_sit_off_the_path() {
        let mut prints = Footprints::new(64, 10.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 4.0, true);
        walk(&mut prints, 0.0, 40.0, 5.0);
        let all: Vec<_> = prints.iter().collect();
        assert!(all.windows(2).all(|w| w[0].left != w[1].left));
        assert!(all[0].left);
        // Левая сторона при движении вправо — вверх экрана
        assert_eq!(all[0].pos, Vec2::new(10.0, -4.0));
        assert_eq!(all[1].pos, Vec2::new(20.0, 4.0));
    }

    #[test]
    fn stamp_places_a_print_where_told_and_respects_capacity() {
        let mut prints = Footprints::new(2, 10.0, 10.0);
        prints.stamp(Vec2::new(5.0, 6.0), RIGHT, true, 0.0, 1.0);
        prints.stamp(Vec2::new(f32::NAN, 0.0), RIGHT, true, 0.0, 1.0);
        let first: Vec<_> = prints.iter().collect();
        assert_eq!(
            (first.len(), first[0].pos, first[0].left),
            (1, Vec2::new(5.0, 6.0), true)
        );
        prints.stamp(Vec2::new(7.0, 6.0), RIGHT, false, 0.0, 1.0);
        prints.stamp(Vec2::new(9.0, 6.0), RIGHT, true, 0.0, 1.0);
        let xs: Vec<f32> = prints.iter().map(|p| p.pos.x).collect();
        assert_eq!(xs, [7.0, 9.0]);
    }

    #[test]
    fn stamp_keeps_clear_of_the_nearest_print() {
        let mut prints = Footprints::new(8, 10.0, 10.0);
        prints.stamp(Vec2::new(10.0, 0.0), RIGHT, true, 4.0, 1.0);
        // Почти в тот же след: отодвигается от него на `clear`
        prints.stamp(Vec2::new(11.0, 0.0), RIGHT, false, 4.0, 1.0);
        // Ровно в след: уходит назад по ходу
        prints.stamp(Vec2::new(10.0, 0.0), RIGHT, true, 4.0, 1.0);
        prints.stamp(Vec2::new(30.0, 0.0), RIGHT, false, 4.0, 1.0);
        let xs: Vec<f32> = prints.iter().map(|p| p.pos.x).collect();
        assert_eq!(xs, [10.0, 14.0, 6.0, 30.0]);
    }

    #[test]
    fn weight_thins_the_print() {
        let mut prints = Footprints::new(4, 10.0, 10.0);
        prints.stamp(Vec2::ZERO, RIGHT, true, 0.0, 0.25);
        prints.stamp(Vec2::new(20.0, 0.0), RIGHT, false, 0.0, 7.0);
        let opacity: Vec<f32> = prints.iter().map(|p| p.opacity).collect();
        assert_eq!(opacity, [0.25, 1.0]);
    }

    #[test]
    fn capacity_replaces_oldest() {
        let mut prints = Footprints::new(3, 10.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        walk(&mut prints, 0.0, 100.0, 10.0);
        let xs: Vec<f32> = prints.iter().map(|p| p.pos.x).collect();
        assert_eq!(xs, [80.0, 90.0, 100.0]);
    }

    #[test]
    fn huge_jump_is_bounded_by_capacity() {
        let mut prints = Footprints::new(4, 10.0, 1.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        prints.track(Vec2::new(1e5, 0.0), RIGHT, 0.0, true);
        let xs: Vec<f32> = prints.iter().map(|p| p.pos.x).collect();
        assert_eq!(xs.len(), 4);
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn prints_expire_after_lifetime() {
        let mut prints = Footprints::new(8, 2.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        prints.track(Vec2::new(10.0, 0.0), RIGHT, 0.0, true);
        prints.update(1.9);
        assert!(!prints.is_empty());
        prints.update(0.1);
        assert!(prints.is_empty());
    }

    #[test]
    fn opacity_is_solid_then_melts_monotonically() {
        let mut prints = Footprints::new(8, 2.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        prints.track(Vec2::new(10.0, 0.0), RIGHT, 0.0, true);
        let mut last = 1.0;
        for _ in 0..39 {
            let opacity = prints.iter().next().map(|p| p.opacity).unwrap();
            assert!(opacity <= last && (0.0..=1.0).contains(&opacity));
            last = opacity;
            prints.update(0.05);
        }
        assert!(last < 0.2);
        let mut fresh = Footprints::new(8, 2.0, 10.0);
        fresh.track(Vec2::ZERO, RIGHT, 0.0, true);
        fresh.track(Vec2::new(10.0, 0.0), RIGHT, 0.0, true);
        fresh.update(1.0);
        assert_eq!(fresh.iter().next().map(|p| p.opacity), Some(1.0));
    }

    #[test]
    fn fading_does_not_depend_on_step_size() {
        let opacity_after = |steps: u32| {
            let mut prints = Footprints::new(8, 2.0, 10.0);
            prints.track(Vec2::ZERO, RIGHT, 0.0, true);
            prints.track(Vec2::new(10.0, 0.0), RIGHT, 0.0, true);
            (0..steps).for_each(|_| prints.update(1.6 / steps as f32));
            prints.iter().next().map(|p| p.opacity).unwrap()
        };
        assert!((opacity_after(1) - opacity_after(160)).abs() < 1e-3);
    }

    #[test]
    fn leaving_the_ground_breaks_the_trail() {
        let mut prints = Footprints::new(8, 10.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        prints.track(Vec2::new(500.0, 0.0), RIGHT, 0.0, false);
        prints.track(Vec2::new(900.0, 0.0), RIGHT, 0.0, true);
        assert!(prints.is_empty());
        prints.track(Vec2::new(910.0, 0.0), RIGHT, 0.0, true);
        assert_eq!(prints.iter().count(), 1);
    }

    #[test]
    fn bounds_cover_negative_coordinates() {
        let mut prints = Footprints::new(8, 10.0, 10.0);
        assert_eq!(prints.bounds(2.0), None);
        let back = Vec2::new(-1.0, 0.0);
        prints.track(Vec2::new(-100.0, 50.0), back, 0.0, true);
        prints.track(Vec2::new(-120.0, 50.0), back, 0.0, true);
        assert_eq!(
            prints.bounds(2.0),
            Some(Rect::new(Vec2::new(-122.0, 48.0), Vec2::new(-108.0, 52.0)))
        );
    }

    #[test]
    fn bad_dt_is_harmless() {
        let mut prints = Footprints::new(8, 2.0, 10.0);
        prints.track(Vec2::ZERO, RIGHT, 0.0, true);
        prints.track(Vec2::new(10.0, 0.0), RIGHT, 0.0, true);
        prints.update(f32::NAN);
        prints.update(-5.0);
        assert_eq!(prints.iter().next().map(|p| p.opacity), Some(1.0));
        prints.update(1e30);
        assert!(prints.is_empty());
    }
}
