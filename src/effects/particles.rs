//! Маленький ограниченный пул частиц: не движок частиц.

use std::f32::consts::{PI, TAU};

use super::{bounding, sane_dt};
use crate::animation::tween::Easing;
use crate::math::{Rect, Vec2};
use crate::rng::Rng;

/// Доля жизни, после которой частица гаснет.
const SOLID: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Burst {
    Dust,
    Sparks,
    Stars,
    Confetti,
    Smoke,
    Alert,
    /// Брызги грязи из лужи.
    Mud,
    /// Облачко чиха.
    Puff,
    /// Зеленоватое облачко «пуф».
    Whiff,
    /// «Z» сна, по одной.
    Zzz,
    Notes,
    /// Комья земли.
    Dirt,
    /// Перья: парят вниз, покачиваясь.
    Feather,
    /// Брызги воды.
    Water,
    /// Кольцо гогота: расходится от клюва.
    Rings,
    Hearts,
    /// Пузырь на луже.
    Bubbles,
}

impl Burst {
    pub const ALL: [Self; 17] = [
        Self::Dust,
        Self::Sparks,
        Self::Stars,
        Self::Confetti,
        Self::Smoke,
        Self::Alert,
        Self::Mud,
        Self::Puff,
        Self::Whiff,
        Self::Zzz,
        Self::Notes,
        Self::Dirt,
        Self::Feather,
        Self::Water,
        Self::Rings,
        Self::Hearts,
        Self::Bubbles,
    ];
}

/// Чем частица рисуется.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Dot,
    Z,
    Note,
    Feather,
    Heart,
    /// Окружность радиусом `radius`.
    Ring,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub pos: Vec2,
    pub radius: f32,
    /// Прямой 0xRRGGBB.
    pub color: u32,
    pub opacity: f32,
    pub shape: Shape,
    /// Наклон, рад: покачивание пера и нотки.
    pub angle: f32,
}

/// Диапазоны `(от, до)`; y растёт вниз, поэтому «вверх» — отрицательные углы.
struct Preset {
    count: usize,
    angle: (f32, f32),
    speed: (f32, f32),
    life: (f32, f32),
    radius: (f32, f32),
    /// Положительная тянет вниз, отрицательная — вверх.
    gravity: f32,
    /// Затухание скорости, 1/с.
    drag: f32,
    colors: &'static [u32],
    shape: Shape,
    /// Покачивание вбок, px.
    sway: f32,
    /// Рост радиуса, px/с.
    grow: f32,
}

const FULL: (f32, f32) = (0.0, TAU);
const UP: (f32, f32) = (-0.85 * PI, -0.15 * PI);
const STRAIGHT_UP: (f32, f32) = (-0.6 * PI, -0.4 * PI);
const SWAY_HZ: f32 = 0.9;
/// Чего нет в пресете.
const PLAIN: Preset = Preset {
    count: 0,
    angle: FULL,
    speed: (0.0, 0.0),
    life: (1.0, 1.0),
    radius: (1.0, 1.0),
    gravity: 0.0,
    drag: 0.0,
    colors: &[0xFFFFFF],
    shape: Shape::Dot,
    sway: 0.0,
    grow: 0.0,
};

// Порядок совпадает с `Burst`
const PRESETS: [Preset; 17] = [
    Preset {
        count: 5,
        angle: FULL,
        speed: (10.0, 35.0),
        life: (0.4, 0.7),
        radius: (3.0, 6.0),
        gravity: -15.0,
        drag: 3.0,
        colors: &[0xB8AFA0, 0xA89F90],
        ..PLAIN
    },
    Preset {
        count: 10,
        angle: FULL,
        speed: (120.0, 260.0),
        life: (0.2, 0.4),
        radius: (1.5, 2.5),
        gravity: 0.0,
        drag: 4.0,
        colors: &[0xFFE066, 0xFFF2B0, 0xFFFFFF],
        ..PLAIN
    },
    Preset {
        count: 6,
        angle: UP,
        speed: (20.0, 60.0),
        life: (0.8, 1.3),
        radius: (3.0, 5.0),
        gravity: -30.0,
        drag: 1.5,
        colors: &[0xFFD700, 0xFFE680],
        ..PLAIN
    },
    Preset {
        count: 16,
        angle: UP,
        speed: (150.0, 320.0),
        life: (1.0, 1.6),
        radius: (2.0, 3.5),
        gravity: 600.0,
        drag: 1.0,
        colors: &[0xF44336, 0x2196F3, 0x4CAF50, 0xFFC107, 0xE040FB],
        ..PLAIN
    },
    Preset {
        count: 6,
        angle: UP,
        speed: (10.0, 30.0),
        life: (1.0, 1.6),
        radius: (5.0, 9.0),
        gravity: -40.0,
        drag: 1.2,
        colors: &[0x3A3A3A, 0x505050, 0x2A2A2A],
        ..PLAIN
    },
    Preset {
        count: 8,
        angle: FULL,
        speed: (100.0, 200.0),
        life: (0.25, 0.45),
        radius: (2.0, 3.0),
        gravity: 0.0,
        drag: 4.0,
        colors: &[0xFFB300, 0xFFC107, 0xFF8F00],
        ..PLAIN
    },
    Preset {
        count: 12,
        angle: UP,
        speed: (70.0, 190.0),
        life: (0.5, 0.9),
        radius: (2.0, 3.5),
        gravity: 520.0,
        drag: 1.5,
        colors: &[0x6B4423, 0x7E5531, 0x5A391E],
        ..PLAIN
    },
    Preset {
        count: 7,
        speed: (20.0, 60.0),
        life: (0.6, 1.0),
        radius: (4.0, 7.0),
        gravity: -20.0,
        drag: 2.5,
        colors: &[0xEEF0F2, 0xDDE1E6],
        ..PLAIN
    },
    Preset {
        count: 6,
        speed: (15.0, 45.0),
        life: (0.9, 1.4),
        radius: (5.0, 8.0),
        gravity: -25.0,
        drag: 2.0,
        colors: &[0xC9D6A3, 0xB5C48A, 0xD8E2B8],
        ..PLAIN
    },
    Preset {
        count: 1,
        angle: STRAIGHT_UP,
        speed: (18.0, 26.0),
        life: (1.6, 2.0),
        radius: (5.5, 6.5),
        gravity: -6.0,
        drag: 0.5,
        colors: &[0x8EA4C8, 0xA7B8D6],
        shape: Shape::Z,
        sway: 10.0,
        ..PLAIN
    },
    Preset {
        count: 2,
        angle: STRAIGHT_UP,
        speed: (30.0, 50.0),
        life: (1.0, 1.4),
        radius: (5.5, 6.5),
        gravity: -20.0,
        drag: 1.0,
        colors: &[0x5C6BC0, 0x7E57C2, 0x26A69A],
        shape: Shape::Note,
        sway: 14.0,
        ..PLAIN
    },
    Preset {
        count: 10,
        angle: UP,
        speed: (60.0, 170.0),
        life: (0.5, 0.8),
        radius: (1.8, 3.2),
        gravity: 560.0,
        drag: 1.5,
        colors: &[0x8D6E4A, 0x7A5C3C, 0x9C7E5A],
        ..PLAIN
    },
    Preset {
        count: 6,
        speed: (30.0, 100.0),
        life: (1.8, 2.6),
        radius: (3.0, 4.5),
        gravity: 40.0,
        drag: 2.5,
        colors: &[0xFFFFFF, 0xF2F4F7],
        shape: Shape::Feather,
        sway: 22.0,
        ..PLAIN
    },
    Preset {
        count: 12,
        angle: UP,
        speed: (80.0, 200.0),
        life: (0.5, 0.8),
        radius: (1.8, 3.0),
        gravity: 560.0,
        drag: 1.5,
        colors: &[0x7DB7E8, 0x9CCBF0, 0xCFE6F8],
        ..PLAIN
    },
    Preset {
        count: 1,
        life: (0.6, 0.6),
        radius: (3.0, 3.0),
        colors: &[0x9AA5B1],
        shape: Shape::Ring,
        grow: 70.0,
        ..PLAIN
    },
    Preset {
        count: 3,
        angle: UP,
        speed: (25.0, 45.0),
        life: (1.0, 1.4),
        radius: (4.5, 6.0),
        gravity: -25.0,
        drag: 1.0,
        colors: &[0xF06292, 0xEC407A],
        shape: Shape::Heart,
        sway: 8.0,
        ..PLAIN
    },
    Preset {
        count: 1,
        angle: STRAIGHT_UP,
        speed: (6.0, 12.0),
        life: (0.8, 1.2),
        radius: (2.5, 3.5),
        gravity: -10.0,
        colors: &[0x8C6844],
        shape: Shape::Ring,
        grow: 4.0,
        ..PLAIN
    },
];

#[derive(Clone, Copy, Debug)]
struct Live {
    pos: Vec2,
    vel: Vec2,
    age: f32,
    life: f32,
    radius: f32,
    color: u32,
    gravity: f32,
    drag: f32,
    shape: Shape,
    sway: f32,
    grow: f32,
    /// Фаза покачивания.
    phase: f32,
}

impl Live {
    fn sway(&self) -> f32 {
        (TAU * SWAY_HZ * self.age + self.phase).sin()
    }
}

#[derive(Clone, Debug)]
pub struct Particles {
    live: Vec<Live>,
    capacity: usize,
}

impl Particles {
    pub fn new(capacity: usize) -> Self {
        Self {
            live: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn burst(&mut self, kind: Burst, origin: Vec2, scale: f32, rng: &mut Rng) {
        let preset = &PRESETS[kind as usize];
        let scale = scale.max(0.0);
        for _ in 0..preset.count.min(self.capacity) {
            let direction = Vec2::from_angle(rng.range(preset.angle.0, preset.angle.1));
            let speed = rng.range(preset.speed.0, preset.speed.1) * scale;
            let color = preset.colors
                [(rng.unit() * preset.colors.len() as f32) as usize % preset.colors.len()];
            if self.live.len() == self.capacity {
                self.live.remove(0);
            }
            self.live.push(Live {
                pos: origin,
                vel: direction * speed,
                age: 0.0,
                life: rng.range(preset.life.0, preset.life.1),
                radius: rng.range(preset.radius.0, preset.radius.1) * scale,
                color,
                gravity: preset.gravity * scale,
                drag: preset.drag,
                shape: preset.shape,
                sway: preset.sway * scale,
                grow: preset.grow * scale,
                phase: rng.range(0.0, TAU),
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = sane_dt(dt);
        self.live.retain_mut(|p| {
            // Не дальше конца жизни: огромный шаг не уносит частицу в бесконечность
            let step = dt.min(p.life - p.age);
            p.age += step;
            p.vel.y += p.gravity * step;
            p.vel = p.vel * (-p.drag * step).exp();
            p.pos += p.vel * step;
            p.radius += p.grow * step;
            p.age < p.life
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = Particle> + '_ {
        self.live.iter().map(|p| Particle {
            pos: p.pos + Vec2::new(p.sway * p.sway(), 0.0),
            radius: p.radius,
            color: p.color,
            opacity: 1.0 - Easing::InQuad.apply((p.age / p.life - SOLID) / (1.0 - SOLID)),
            shape: p.shape,
            angle: 0.5 * (TAU * SWAY_HZ * p.age + p.phase).cos() * p.sway.signum(),
        })
    }

    pub fn bounds(&self) -> Option<Rect> {
        bounding(self.live.iter().map(|p| (p.pos, p.radius + p.sway)))
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Burst; 17] = Burst::ALL;

    #[test]
    fn capacity_is_never_exceeded() {
        let mut rng = Rng::new(1);
        let mut particles = Particles::new(20);
        for i in 0..50 {
            particles.burst(ALL[i % 6], Vec2::ZERO, 1.0, &mut rng);
            assert!(particles.iter().count() <= 20);
        }
        assert_eq!(particles.iter().count(), 20);
        let mut tiny = Particles::new(3);
        tiny.burst(Burst::Confetti, Vec2::ZERO, 1.0, &mut rng);
        assert_eq!(tiny.iter().count(), 3);
        let mut none = Particles::new(0);
        none.burst(Burst::Confetti, Vec2::ZERO, 1.0, &mut rng);
        assert!(none.is_empty());
    }

    #[test]
    fn everything_expires() {
        for kind in ALL {
            let mut particles = Particles::new(32);
            particles.burst(kind, Vec2::new(10.0, 10.0), 1.0, &mut Rng::new(2));
            assert!(!particles.is_empty());
            (0..60).for_each(|_| particles.update(0.05));
            assert!(particles.is_empty(), "{kind:?}");
        }
    }

    #[test]
    fn same_seed_same_burst() {
        let run = || {
            let mut particles = Particles::new(32);
            let mut rng = Rng::new(9);
            particles.burst(Burst::Confetti, Vec2::new(5.0, 5.0), 1.5, &mut rng);
            particles.update(0.2);
            particles.iter().collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn bounds_contain_every_particle() {
        let mut particles = Particles::new(32);
        assert_eq!(particles.bounds(), None);
        let mut rng = Rng::new(4);
        particles.burst(Burst::Sparks, Vec2::new(-300.0, -200.0), 2.0, &mut rng);
        particles.burst(Burst::Smoke, Vec2::new(-310.0, -190.0), 1.0, &mut rng);
        particles.update(0.1);
        let bounds = particles.bounds().unwrap();
        for p in particles.iter() {
            assert!(p.pos.x - p.radius >= bounds.min.x && p.pos.x + p.radius <= bounds.max.x);
            assert!(p.pos.y - p.radius >= bounds.min.y && p.pos.y + p.radius <= bounds.max.y);
        }
    }

    #[test]
    fn opacity_stays_in_unit_range_and_ends_faded() {
        let mut particles = Particles::new(32);
        particles.burst(Burst::Stars, Vec2::ZERO, 1.0, &mut Rng::new(5));
        for _ in 0..30 {
            assert!(particles.iter().all(|p| (0.0..=1.0).contains(&p.opacity)));
            particles.update(0.04);
        }
        assert!(particles.iter().all(|p| p.opacity < 0.7));
    }

    #[test]
    fn gravity_points_the_right_way() {
        let vy = |particles: &Particles| particles.live.iter().map(|p| p.vel.y).collect::<Vec<_>>();
        for (kind, down) in [(Burst::Confetti, true), (Burst::Smoke, false)] {
            let mut particles = Particles::new(32);
            particles.burst(kind, Vec2::ZERO, 1.0, &mut Rng::new(6));
            let before = vy(&particles);
            particles.update(0.01);
            let after = vy(&particles);
            assert!(
                before.iter().zip(&after).all(|(b, a)| (a > b) == down),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn bad_dt_is_harmless() {
        let mut particles = Particles::new(32);
        particles.burst(Burst::Dust, Vec2::new(1.0, 1.0), 1.0, &mut Rng::new(7));
        let before: Vec<_> = particles.iter().collect();
        particles.update(f32::NAN);
        particles.update(-3.0);
        assert_eq!(particles.iter().collect::<Vec<_>>(), before);
        particles.update(f32::INFINITY);
        assert!(particles.is_empty());
        particles.burst(Burst::Dust, Vec2::new(1.0, 1.0), 1.0, &mut Rng::new(7));
        particles.update(1e30);
        assert!(particles.is_empty());
    }
}
