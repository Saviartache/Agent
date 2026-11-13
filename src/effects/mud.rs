//! Лужи грязи и воды: разливаются, ждут персонажа и сохнут, сжимаясь. Плоские — ни высоты, ни тени;
//! гаснут размером, а не прозрачностью, чтобы перекрытые доли не темнели.

use std::f32::consts::TAU;

use super::{bounding, sane_dt};
use crate::animation::tween::Easing;
use crate::math::{Rect, Vec2};
use crate::rng::Rng;

/// Лужа — середина и доли вокруг: неровный край без контура.
pub const LOBES: usize = 6;
const GROW_SECS: f32 = 0.6;
const DRY_SECS: f32 = 3.0;
/// Глубина лужи на экране сжата, как у всего, что лежит на земле.
const GROUND_SQUASH: f32 = 0.55;
/// Доли: удаление от середины и размер — доли радиуса лужи.
const LOBE_REACH: [f32; 2] = [0.45, 0.7];
const LOBE_SIZE: [f32; 2] = [0.4, 0.62];

/// Эллипс доли относительно середины лужи полного размера.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lobe {
    pub offset: Vec2,
    pub radii: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Liquid {
    #[default]
    Mud,
    Water,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puddle {
    pub center: Vec2,
    pub liquid: Liquid,
    /// 0…1: разливается и сохнет размером.
    pub size: f32,
    pub lobes: [Lobe; LOBES],
}

impl Puddle {
    /// Доли на экране: середина и полуоси.
    pub fn ellipses(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.lobes.iter().map(|lobe| {
            (
                self.center + lobe.offset * self.size,
                lobe.radii * self.size,
            )
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Live {
    center: Vec2,
    liquid: Liquid,
    radius: f32,
    lobes: [Lobe; LOBES],
    age: f32,
    life: f32,
}

#[derive(Clone, Debug)]
pub struct Mud {
    live: Vec<Live>,
    capacity: usize,
}

impl Mud {
    pub fn new(capacity: usize) -> Self {
        Self {
            live: Vec::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    /// Лужа грязи радиусом `radius` px; если в неё никто не залез, сохнет через `ttl` секунд.
    pub fn pour(&mut self, center: Vec2, radius: f32, ttl: f32, rng: &mut Rng) {
        self.pour_liquid(center, radius, ttl, Liquid::Mud, rng);
    }

    pub fn pour_liquid(
        &mut self,
        center: Vec2,
        radius: f32,
        ttl: f32,
        liquid: Liquid,
        rng: &mut Rng,
    ) {
        let sane = center.x.is_finite() && center.y.is_finite() && radius.is_finite();
        if !sane || radius <= 0.0 {
            return;
        }
        let ground = |v: Vec2| Vec2::new(v.x, v.y * GROUND_SQUASH);
        let mut lobes = [Lobe {
            offset: Vec2::ZERO,
            radii: ground(Vec2::new(radius, radius)) * 0.7,
        }; LOBES];
        let turn = rng.range(0.0, TAU);
        for (i, lobe) in lobes.iter_mut().enumerate().skip(1) {
            let angle = turn + i as f32 * TAU / (LOBES - 1) as f32 + rng.range(-0.3, 0.3);
            let reach = rng.range(LOBE_REACH[0], LOBE_REACH[1]) * radius;
            let size = rng.range(LOBE_SIZE[0], LOBE_SIZE[1]) * radius;
            *lobe = Lobe {
                offset: ground(Vec2::from_angle(angle) * reach),
                radii: ground(Vec2::new(size * 1.2, size)),
            };
        }
        if self.live.len() == self.capacity {
            self.live.remove(0);
        }
        self.live.push(Live {
            center,
            liquid,
            radius,
            lobes,
            age: 0.0,
            life: ttl.max(GROW_SECS + DRY_SECS),
        });
    }

    /// Лужи под `at` начинают сохнуть: в них уже вывалялись.
    pub fn dry_at(&mut self, at: Vec2) {
        for puddle in &mut self.live {
            if puddle.center.distance(at) <= puddle.radius {
                puddle.life = puddle.life.min(puddle.age + DRY_SECS);
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = sane_dt(dt);
        self.live.retain_mut(|puddle| {
            puddle.age = (puddle.age + dt).min(puddle.life);
            puddle.age < puddle.life
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = Puddle> + '_ {
        self.live.iter().map(|puddle| {
            let grow = puddle.age / GROW_SECS;
            let dry = (puddle.life - puddle.age) / DRY_SECS;
            Puddle {
                center: puddle.center,
                liquid: puddle.liquid,
                size: Easing::OutQuad.apply(grow.min(dry).clamp(0.0, 1.0)),
                lobes: puddle.lobes,
            }
        })
    }

    pub fn bounds(&self) -> Option<Rect> {
        // Доля не выходит дальше полутора радиусов
        bounding(
            self.live
                .iter()
                .map(|puddle| (puddle.center, puddle.radius * 1.5)),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Vec2 = Vec2::new(-200.0, 300.0);
    const DT: f32 = 1.0 / 60.0;

    fn poured(ttl: f32) -> Mud {
        let mut mud = Mud::new(2);
        mud.pour(AT, 30.0, ttl, &mut Rng::new(3));
        mud
    }

    fn run(mud: &mut Mud, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            mud.update(DT);
        }
    }

    fn size(mud: &Mud) -> f32 {
        mud.iter().next().map_or(0.0, |puddle| puddle.size)
    }

    #[test]
    fn spreads_waits_and_dries_by_itself() {
        let mut mud = poured(10.0);
        assert_eq!(size(&mud), 0.0);
        run(&mut mud, 1.0);
        assert_eq!(size(&mud), 1.0);
        run(&mut mud, 6.0);
        assert_eq!(size(&mud), 1.0);
        run(&mut mud, 1.5);
        assert!(size(&mud) < 1.0);
        run(&mut mud, 2.0);
        assert!(mud.is_empty());
    }

    #[test]
    fn wallowing_dries_only_the_puddle_underfoot() {
        let mut mud = poured(60.0);
        mud.pour(AT + Vec2::new(500.0, 0.0), 30.0, 60.0, &mut Rng::new(4));
        run(&mut mud, 1.0);
        mud.dry_at(AT + Vec2::new(5.0, 5.0));
        run(&mut mud, DRY_SECS + 0.1);
        let left: Vec<Vec2> = mud.iter().map(|puddle| puddle.center).collect();
        assert_eq!(left, [AT + Vec2::new(500.0, 0.0)]);
    }

    #[test]
    fn bounds_hold_every_lobe_and_capacity_holds() {
        let mut mud = Mud::new(1);
        mud.pour(AT, 30.0, 60.0, &mut Rng::new(1));
        mud.pour(AT, 30.0, 60.0, &mut Rng::new(2));
        assert_eq!(mud.iter().count(), 1);
        run(&mut mud, 1.0);
        let bounds = mud.bounds().unwrap();
        for puddle in mud.iter() {
            for (center, radii) in puddle.ellipses() {
                let reach = radii.x.max(radii.y);
                assert!(bounds.contains(center + Vec2::new(reach, 0.0)));
                assert!(bounds.contains(center - Vec2::new(reach, 0.0)));
            }
        }
    }

    #[test]
    fn broken_input_is_harmless() {
        let mut mud = Mud::new(2);
        mud.pour(Vec2::new(f32::NAN, 0.0), 30.0, 10.0, &mut Rng::new(1));
        mud.pour(AT, f32::NAN, 10.0, &mut Rng::new(1));
        assert!(mud.is_empty());
        let mut mud = poured(10.0);
        mud.update(f32::NAN);
        mud.update(-1.0);
        mud.update(1e9);
        assert!(mud.is_empty());
    }
}
