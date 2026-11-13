//! Уши-лопухи слоника: плоские эллипсы на шарнире у затылка. Раскрываются на ходу,
//! в прыжке и по накладке (`Antic::wings`/`flap`), а на пружине отстают от поворота
//! головы и разгона тела.

use std::f32::consts::TAU;

use super::pose::Ear;
use crate::rig::vec3::{Frame, Vec3};

/// Форма уха вида: доли радиуса головы и радианы.
pub struct EarShape {
    /// Шарнир: вбок, вперёд, вверх.
    pub root: [f32; 3],
    /// Полуоси лопуха: от шарнира наружу и вниз.
    pub radii: [f32; 2],
    /// Раскрытие от прижатого назад: в покое и целиком.
    pub open: [f32; 2],
}

/// Центр лопуха от шарнира в долях полуосей: наружу и вниз.
const REACH: f32 = 0.85;
const DROP: f32 = 0.6;
/// Сдвиг низа лопуха наружу в долях полуоси вниз.
const SLANT: f32 = 0.35;
/// Насколько ухо висит к земле, а не от макушки: лопух мягкий.
const FLOP: f32 = 0.7;
/// Раскрыты целиком при такой разнице высот задних ступней и таком прыжке, design px.
const STRIDE: f32 = 7.0;
const HOP: f32 = 6.0;
/// Взмах ходит от этой доли раскрытия до полного.
const FLAP_BASE: f32 = 0.6;
/// Пружина: частота, рад/с, и затухание (меньше 1 — перелёт).
const OMEGA: f32 = 16.0;
const DAMPING: f32 = 0.4;
/// Поворот головы, рад/с, раскрывает ухо снаружи поворота и прижимает внутреннее.
const SWING: f32 = 0.12;
/// Разгон вперёд, design px/с², прижимает уши назад, торможение раскрывает.
const INERTIA: f32 = 0.0015;
/// Дальше этого пружина ухо не уводит, рад.
const MAX_LAG: f32 = 0.6;
const SIDES: [f32; 2] = [-1.0, 1.0];

/// Насколько уши раскрыты, 0…1: `stride` — разница высот задних ступней, `hop` —
/// прыжок, оба design px; `wings` и `flap` — накладка.
pub fn spread(stride: f32, hop: f32, wings: f32, flap: f32) -> f32 {
    let flap = wings * (FLAP_BASE + (1.0 - FLAP_BASE) * (TAU * flap).sin());
    (stride / STRIDE).max(hop / HOP).max(flap).clamp(0.0, 1.0)
}

/// Раскрытие левого и правого уха на пружине, рад.
#[derive(Clone, Copy, Debug)]
pub struct Flaps {
    open: [f32; 2],
    vel: [f32; 2],
}

impl Flaps {
    pub fn resting(open: f32) -> Self {
        Self {
            open: [open; 2],
            vel: [0.0; 2],
        }
    }

    /// `want` — раскрытие, рад; `turn` — поворот головы, рад/с; `push` — разгон
    /// вперёд, design px/с².
    pub fn update(&mut self, dt: f32, want: f32, turn: f32, push: f32) {
        for (i, side) in SIDES.into_iter().enumerate() {
            let lag = (SWING * turn * side - INERTIA * push).clamp(-MAX_LAG, MAX_LAG);
            let pull = (want + lag - self.open[i]) * (OMEGA * OMEGA);
            self.vel[i] += (pull - self.vel[i] * (2.0 * DAMPING * OMEGA)) * dt;
            self.open[i] += self.vel[i] * dt;
            // Мусор на входе: ухо прижато
            if !(self.open[i].is_finite() && self.vel[i].is_finite()) {
                (self.open[i], self.vel[i]) = (0.0, 0.0);
            }
        }
    }

    /// Уши на голове с центром `center` и осями `head`.
    pub fn place(&self, shape: &EarShape, head: &Frame, center: Vec3, radius: f32) -> [Ear; 2] {
        std::array::from_fn(|i| {
            let side = head.right * SIDES[i];
            let (sin, cos) = self.open[i].sin_cos();
            // Шарнир у переднего края: прижатое ухо лежит назад изнанкой к голове
            let blade = side * sin - head.forward * cos;
            let hang = hang(blade, head.up);
            let inner = blade.cross(hang) * SIDES[i];
            let [x, y, z] = shape.root;
            let root = center + (side * x + head.forward * y + head.up * z) * radius;
            // Низ лопуха отходит наружу: лист висит, а не тарелка
            let down = blade * SLANT + hang;
            let axes = [
                blade * (shape.radii[0] * radius),
                down * (shape.radii[1] * radius),
            ];
            grounded(Ear {
                root,
                center: root + axes[0] * REACH + axes[1] * DROP,
                axes,
                inner,
            })
        })
    }
}

/// Куда ухо висит поперёк `blade`: вниз от макушки, но мягкое — больше к земле.
fn hang(blade: Vec3, up: Vec3) -> Vec3 {
    let down = (up * -1.0).lerp(Vec3::new(0.0, 0.0, -1.0), FLOP);
    let hang = (down - blade * down.dot(blade)).normalized();
    if hang == Vec3::ZERO { up * -1.0 } else { hang }
}

/// Ухо не уходит под землю: ниже шарнира сплющивается по высоте и ложится на неё.
fn grounded(ear: Ear) -> Ear {
    let reach = ear.axes[0].z.hypot(ear.axes[1].z);
    let below = ear.root.z - ear.center.z + reach;
    if below <= ear.root.z || below <= 0.0 {
        return ear;
    }
    let k = (ear.root.z / below).max(0.0);
    let squash = |v: Vec3| Vec3::new(v.x, v.y, v.z * k);
    let n = ear.inner;
    Ear {
        root: ear.root,
        center: ear.root + squash(ear.center - ear.root),
        axes: ear.axes.map(squash),
        // Нормаль при сжатии по высоте наклоняется к отвесу
        inner: Vec3::new(n.x * k, n.y * k, n.z).normalized(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: EarShape = EarShape {
        root: [0.7, -0.3, 0.3],
        radii: [0.6, 0.8],
        open: [0.5, 1.5],
    };

    fn settle(want: f32, turn: f32, push: f32) -> Flaps {
        let mut flaps = Flaps::resting(SHAPE.open[0]);
        for _ in 0..240 {
            flaps.update(1.0 / 120.0, want, turn, push);
        }
        flaps
    }

    #[test]
    fn spreads_from_stride_hop_and_flap() {
        assert_eq!(spread(0.0, 0.0, 0.0, 0.0), 0.0);
        assert_eq!(spread(STRIDE, 0.0, 0.0, 0.0), 1.0);
        assert_eq!(spread(0.0, HOP * 2.0, 0.0, 0.0), 1.0);
        assert_eq!(spread(0.0, 0.0, 1.0, 0.25), 1.0);
        assert!(spread(0.0, 0.0, 1.0, 0.75) < 0.5);
    }

    #[test]
    fn spread_ear_turns_its_inner_side_back_and_sticks_out() {
        let head = Frame::new(0.0, 0.0, 0.0);
        let [calm, _] = settle(SHAPE.open[0], 0.0, 0.0).place(&SHAPE, &head, Vec3::ZERO, 10.0);
        let [open, _] = settle(SHAPE.open[1], 0.0, 0.0).place(&SHAPE, &head, Vec3::ZERO, 10.0);
        // Левое ухо: наружу — против `right`
        let out = |ear: &Ear| -ear.center.dot(head.right);
        assert!(out(&open) > out(&calm) + 2.0);
        assert!(
            calm.inner.dot(head.right) > 0.5,
            "прижатое — изнанкой к голове"
        );
        assert!(
            open.inner.dot(head.forward) < -0.9,
            "раскрытое — изнанкой назад"
        );
        assert!(calm.axes[0].dot(calm.inner).abs() < 1e-4);
    }

    #[test]
    fn ears_lag_behind_turns_and_starts() {
        let want = SHAPE.open[0];
        let still = settle(want, 0.0, 0.0);
        let turning = settle(want, 3.0, 0.0);
        assert!(
            turning.open[1] > still.open[1] + 0.2,
            "внешнее к повороту раскрыто"
        );
        assert!(turning.open[0] < still.open[0] - 0.2);
        let starting = settle(want, 0.0, 300.0);
        assert!(starting.open.iter().all(|a| *a < want - 0.2));
    }

    #[test]
    fn garbage_settles_back() {
        let mut flaps = Flaps::resting(0.5);
        flaps.update(1.0 / 120.0, f32::NAN, 0.0, 0.0);
        assert_eq!(flaps.open, [0.0; 2]);
    }
}
