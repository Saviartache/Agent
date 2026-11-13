//! Листья: лежат кучей, пока в неё не вбегут, потом разлетаются, ложатся и тают.
//! Куча — не отдельный объект, а те же листья в покое.

use std::f32::consts::TAU;

use super::{bounding, sane_dt};
use crate::math::{Rect, Vec2};
use crate::rng::Rng;

/// Прямые 0xRRGGBB.
const COLORS: [u32; 5] = [0xD9822B, 0xC8501E, 0xE2B13C, 0x9C4A1A, 0xB7372A];
/// Половина длины листа, логические px.
const SIZE: [f32; 2] = [4.0, 6.5];
const FADE_IN: f32 = 0.3;
const FADE_OUT: f32 = 0.8;
/// Разлёт: скорость от центра кучи и доля толчка, которая достаётся листу, px/с.
const BLAST: [f32; 2] = [120.0, 520.0];
const PUSH: [f32; 2] = [0.4, 1.3];
/// Затухание полёта и вращения, 1/с.
const DRAG: f32 = 2.4;
/// Вращение в полёте, рад/с.
const SPIN: f32 = 14.0;
/// Столько лист лежит после разлёта, с.
const REST_SECS: [f32; 2] = [4.0, 7.0];

/// Высота горки в середине кучи, логические px.
const PEAK: f32 = 18.0;
/// Глубина кучи на экране сжата, как у всего, на что смотрят сверху под углом.
const GROUND_SQUASH: f32 = 0.55;
/// Подброс при ударе и падение обратно, px/с и px/с².
const TOSS: [f32; 2] = [60.0, 260.0];
const GRAVITY: f32 = 700.0;
/// Тень листа чуть шире его самого.
pub const SHADOW_GROW: f32 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf {
    /// Точка на земле под листом: здесь лежит его тень.
    pub pos: Vec2,
    /// Высота над землёй: лист рисуется выше своей тени на столько px.
    pub lift: f32,
    pub angle: f32,
    /// Полуоси: вдоль и поперёк.
    pub radii: Vec2,
    /// Прямой 0xRRGGBB.
    pub color: u32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug)]
struct Live {
    pos: Vec2,
    vel: Vec2,
    lift: f32,
    /// Скорость вверх.
    rise: f32,
    /// Ноль, пока лист не подброшен.
    gravity: f32,
    angle: f32,
    spin: f32,
    age: f32,
    life: f32,
    size: f32,
    color: u32,
}

#[derive(Clone, Debug)]
pub struct Leaves {
    live: Vec<Live>,
    capacity: usize,
}

impl Leaves {
    pub fn new(capacity: usize) -> Self {
        Self {
            live: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Куча из `count` листьев вокруг `center`; нетронутая тает через `ttl` секунд.
    pub fn heap(
        &mut self,
        center: Vec2,
        radius: f32,
        count: usize,
        ttl: f32,
        scale: f32,
        rng: &mut Rng,
    ) {
        if !(center.x.is_finite() && center.y.is_finite()) {
            return;
        }
        let scale = scale.max(0.0);
        let count = count.min(self.capacity);
        for _ in 0..count {
            // Корень из равномерного дал бы ровный диск; без корня листья гуще к середине
            let spread = rng.unit();
            let offset = Vec2::from_angle(rng.range(0.0, TAU)) * (radius * spread);
            if self.live.len() == self.capacity {
                self.live.remove(0);
            }
            self.live.push(Live {
                // Куча приплюснута: на неё смотрят сверху под углом
                pos: center + Vec2::new(offset.x, offset.y * GROUND_SQUASH),
                vel: Vec2::ZERO,
                // Горка: к середине листья лежат всё выше друг на друге
                lift: PEAK * scale * (1.0 - spread * spread) * rng.range(0.25, 1.0),
                rise: 0.0,
                gravity: 0.0,
                angle: rng.range(0.0, TAU),
                spin: 0.0,
                age: 0.0,
                life: ttl.max(FADE_IN + FADE_OUT),
                size: rng.range(SIZE[0], SIZE[1]) * scale,
                color: COLORS[(rng.unit() * COLORS.len() as f32) as usize % COLORS.len()],
            });
        }
        // Дальние и нижние рисуются первыми: ближние и верхние их перекрывают
        let heap = self.live.len() - count;
        self.live[heap..]
            .sort_unstable_by(|a, b| (a.pos.y + a.lift).total_cmp(&(b.pos.y + b.lift)));
    }

    /// Листья в круге разлетаются: от точки удара и по ходу `push`. Возвращает, сколько их было.
    pub fn kick(&mut self, at: Vec2, radius: f32, push: Vec2, scale: f32, rng: &mut Rng) -> usize {
        let push = if push.x.is_finite() && push.y.is_finite() {
            push
        } else {
            Vec2::ZERO
        };
        let mut kicked = 0;
        for leaf in &mut self.live {
            let away = leaf.pos - at;
            if away.length() > radius {
                continue;
            }
            let direction = if away.length() > f32::EPSILON {
                away.normalized()
            } else {
                Vec2::from_angle(rng.range(0.0, TAU))
            };
            leaf.vel = direction * (rng.range(BLAST[0], BLAST[1]) * scale.max(0.0))
                + push * rng.range(PUSH[0], PUSH[1]);
            leaf.spin = rng.range(-SPIN, SPIN);
            leaf.rise = rng.range(TOSS[0], TOSS[1]) * scale.max(0.0);
            leaf.gravity = GRAVITY * scale.max(0.0);
            leaf.life = leaf.age + rng.range(REST_SECS[0], REST_SECS[1]);
            kicked += 1;
        }
        kicked
    }

    pub fn update(&mut self, dt: f32) {
        let dt = sane_dt(dt);
        self.live.retain_mut(|leaf| {
            // Не дальше конца жизни: огромный шаг не уносит лист в бесконечность
            let step = dt.min(leaf.life - leaf.age);
            leaf.age += step;
            let slow = (-DRAG * step).exp();
            // Путь за шаг при затухающей скорости — точный интеграл, а не шаг Эйлера
            leaf.pos += leaf.vel * ((1.0 - slow) / DRAG);
            leaf.angle = (leaf.angle + leaf.spin * ((1.0 - slow) / DRAG)) % TAU;
            leaf.vel = leaf.vel * slow;
            leaf.spin *= slow;
            // Подброшенный лист падает; лежащий в куче (`gravity == 0`) держит свою высоту
            leaf.rise -= leaf.gravity * step;
            leaf.lift += leaf.rise * step;
            if leaf.lift <= 0.0 {
                (leaf.lift, leaf.rise, leaf.gravity) = (0.0, 0.0, 0.0);
            }
            leaf.age < leaf.life
        });
    }

    /// Сколько листьев лежит в круге: цела ли ещё куча.
    pub fn count_within(&self, at: Vec2, radius: f32) -> usize {
        let near = |leaf: &&Live| leaf.pos.distance(at) <= radius;
        self.live.iter().filter(near).count()
    }

    pub fn iter(&self) -> impl Iterator<Item = Leaf> + '_ {
        self.live.iter().map(|leaf| Leaf {
            pos: leaf.pos,
            lift: leaf.lift,
            angle: leaf.angle,
            radii: Vec2::new(leaf.size, leaf.size * 0.5),
            color: leaf.color,
            opacity: (leaf.age / FADE_IN).min(1.0)
                * ((leaf.life - leaf.age) / FADE_OUT).clamp(0.0, 1.0),
        })
    }

    pub fn bounds(&self) -> Option<Rect> {
        // И лист на своей высоте, и его тень на земле
        let lifted = |leaf: &Live| (leaf.pos - Vec2::new(0.0, leaf.lift), leaf.size);
        let grounded = |leaf: &Live| (leaf.pos, leaf.size * SHADOW_GROW);
        bounding(
            self.live
                .iter()
                .flat_map(|leaf| [lifted(leaf), grounded(leaf)]),
        )
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

    const CENTER: Vec2 = Vec2::new(-300.0, 200.0);
    const DT: f32 = 1.0 / 60.0;

    fn heaped(count: usize) -> (Leaves, Rng) {
        let mut rng = Rng::new(5);
        let mut leaves = Leaves::new(64);
        leaves.heap(CENTER, 30.0, count, 10.0, 1.0, &mut rng);
        (leaves, rng)
    }

    fn run(leaves: &mut Leaves, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            leaves.update(DT);
        }
    }

    #[test]
    fn heap_lies_still_inside_its_radius() {
        let (mut leaves, _) = heaped(40);
        let before: Vec<_> = leaves.iter().map(|leaf| leaf.pos).collect();
        run(&mut leaves, 2.0);
        let after: Vec<_> = leaves.iter().map(|leaf| leaf.pos).collect();
        assert_eq!(before, after);
        assert_eq!(leaves.count_within(CENTER, 30.0), 40);
    }

    #[test]
    fn untouched_heap_melts_after_its_ttl() {
        let (mut leaves, _) = heaped(40);
        run(&mut leaves, 9.5);
        assert!(leaves.iter().all(|leaf| leaf.opacity < 1.0));
        run(&mut leaves, 1.0);
        assert!(leaves.is_empty());
    }

    #[test]
    fn kick_scatters_the_heap_along_the_push_and_leaves_come_to_rest() {
        let (mut leaves, mut rng) = heaped(40);
        let push = Vec2::new(200.0, 0.0);
        assert_eq!(leaves.kick(CENTER, 30.0, push, 1.0, &mut rng), 40);
        run(&mut leaves, 3.0);
        assert!(leaves.count_within(CENTER, 30.0) < 10);
        let mean = leaves.iter().map(|leaf| leaf.pos.x).sum::<f32>() / 40.0;
        assert!(mean > CENTER.x + 40.0, "{mean}");
        let resting: Vec<_> = leaves.iter().map(|leaf| leaf.pos).collect();
        run(&mut leaves, 0.5);
        let later: Vec<_> = leaves.iter().map(|leaf| leaf.pos).collect();
        assert!(
            resting
                .iter()
                .zip(&later)
                .all(|(a, b)| a.distance(*b) < 1.0)
        );
        run(&mut leaves, 8.0);
        assert!(leaves.is_empty());
    }

    #[test]
    fn kick_outside_the_heap_touches_nothing() {
        let (mut leaves, mut rng) = heaped(10);
        let far = CENTER + Vec2::new(500.0, 0.0);
        assert_eq!(leaves.kick(far, 30.0, Vec2::ZERO, 1.0, &mut rng), 0);
    }

    #[test]
    fn capacity_is_never_exceeded_and_bounds_hold_every_leaf() {
        let mut rng = Rng::new(1);
        let mut leaves = Leaves::new(16);
        for _ in 0..5 {
            leaves.heap(CENTER, 30.0, 40, 10.0, 1.0, &mut rng);
        }
        assert_eq!(leaves.iter().count(), 16);
        leaves.kick(CENTER, 30.0, Vec2::new(0.0, -300.0), 1.0, &mut rng);
        run(&mut leaves, 1.0);
        let bounds = leaves.bounds().unwrap();
        assert!(
            leaves
                .iter()
                .all(|leaf| bounds.inflate(0.01).contains(leaf.pos))
        );
    }

    #[test]
    fn flight_does_not_depend_on_step_size() {
        let fly = |dt: f32| {
            let (mut leaves, mut rng) = heaped(20);
            leaves.kick(CENTER, 30.0, Vec2::new(150.0, 50.0), 1.0, &mut rng);
            for _ in 0..(1.0 / dt).round() as usize {
                leaves.update(dt);
            }
            leaves.iter().map(|leaf| leaf.pos).collect::<Vec<_>>()
        };
        let (fine, coarse) = (fly(1.0 / 240.0), fly(1.0 / 30.0));
        assert!(fine.iter().zip(&coarse).all(|(a, b)| a.distance(*b) < 0.5));
    }

    #[test]
    fn broken_input_is_harmless() {
        let (mut leaves, mut rng) = heaped(5);
        leaves.heap(Vec2::new(f32::NAN, 0.0), 30.0, 5, 10.0, 1.0, &mut rng);
        leaves.kick(CENTER, 30.0, Vec2::new(f32::NAN, 0.0), 1.0, &mut rng);
        leaves.update(f32::NAN);
        leaves.update(-1.0);
        leaves.update(1e9);
        assert!(leaves.is_empty());
    }
}
