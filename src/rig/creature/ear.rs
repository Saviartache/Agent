//! Уши-лопухи слоника: плоские эллипсы на шарнире у затылка, изнанкой вперёд. Раскрываются
//! на ходу, в прыжке и по накладке (`Antic::wings`/`flap`), на пружине отстают от поворота
//! головы и разгона тела, во сне обвисают, торчком и под землю не уходят.

use std::f32::consts::TAU;

use super::pose::Ear;
use crate::rig::vec3::{Frame, Vec3};

/// Форма уха вида: доли радиуса головы и радианы.
pub struct EarShape {
    /// Шарнир: вбок, вперёд, вверх.
    pub root: [f32; 3],
    /// Полуоси лопуха: от шарнира наружу и вниз.
    pub radii: [f32; 2],
    /// Угол от прижатого назад: в покое (больше прямого — чашей к морде) и раскрытое.
    pub open: [f32; 2],
    /// Во сне уши обвисают: шарнир ниже на столько.
    pub slump: f32,
}

/// Центр лопуха от шарнира в долях полуосей: наружу и вниз.
const REACH: f32 = 0.85;
const DROP: f32 = 0.6;
/// Сдвиг низа лопуха наружу в долях полуоси вниз.
const SLANT: f32 = 0.35;
/// Раскрытое целиком отводит низ наружу ещё на столько.
const FAN: f32 = 0.75;
/// Насколько ухо висит к земле, а не от макушки: лопух мягкий.
const FLOP: f32 = 0.5;
/// Выше шарнира ухо поднимается не больше этой доли полуоси вниз.
const LIFT: f32 = 0.6;
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
            let lag = (-(SWING * turn * side) - INERTIA * push).clamp(-MAX_LAG, MAX_LAG);
            let pull = (want + lag - self.open[i]) * (OMEGA * OMEGA);
            self.vel[i] += (pull - self.vel[i] * (2.0 * DAMPING * OMEGA)) * dt;
            self.open[i] += self.vel[i] * dt;
            // Мусор на входе: ухо прижато
            if !(self.open[i].is_finite() && self.vel[i].is_finite()) {
                (self.open[i], self.vel[i]) = (0.0, 0.0);
            }
        }
    }

    /// Уши на голове с центром `center` и осями `head`; `tuck` — глубина сна.
    pub fn place(
        &self,
        shape: &EarShape,
        head: &Frame,
        center: Vec3,
        radius: f32,
        tuck: f32,
    ) -> [Ear; 2] {
        let back = Vec3::new(-head.forward.x, -head.forward.y, 0.0).normalized();
        std::array::from_fn(|i| {
            let side = head.right * SIDES[i];
            let (sin, cos) = self.open[i].sin_cos();
            // Угол от прижатого назад; за прямым ухо загибается к морде
            let blade = side * sin - head.forward * cos;
            let hang = hang(blade, head.up);
            // Изнанка смотрит вперёд: до прямого угла — чуть наружу, за ним — к морде
            let inner = hang.cross(blade) * SIDES[i];
            let [x, y, z] = shape.root;
            let z = z - shape.slump * tuck.clamp(0.0, 1.0);
            let root = center + (side * x + head.forward * y + head.up * z) * radius;
            // Низ лопуха отходит наружу: лист висит, а не тарелка; раскрытый — веером
            let spread = (shape.open[0] - self.open[i]) / (shape.open[0] - shape.open[1]);
            let down = blade * (SLANT + FAN * spread.clamp(0.0, 1.0)) + hang;
            let axes = [
                blade * (shape.radii[0] * radius),
                down * (shape.radii[1] * radius),
            ];
            let ear = Ear {
                root,
                center: root + axes[0] * REACH + axes[1] * DROP,
                axes,
                inner,
            };
            grounded(fallen(ear, back), back)
        })
    }
}

/// Куда ухо висит поперёк `blade`: вниз от макушки, но мягкое — больше к земле.
fn hang(blade: Vec3, up: Vec3) -> Vec3 {
    let down = (up * -1.0).lerp(Vec3::new(0.0, 0.0, -1.0), FLOP);
    let hang = (down - blade * down.dot(blade)).normalized();
    if hang == Vec3::ZERO { up * -1.0 } else { hang }
}

/// Мягкое ухо торчком не стоит: поднятое над шарниром выше `LIFT` своей полуоси вниз
/// валится назад, на голову (на боку).
fn fallen(ear: Ear, back: Vec3) -> Ear {
    let reach = ear.axes[0].z.hypot(ear.axes[1].z);
    let rise = ear.center.z - ear.root.z + reach;
    let limit = LIFT * ear.axes[1].length();
    if rise <= limit {
        return ear;
    }
    let k = limit / rise;
    flattened(ear, k, |v| {
        Vec3::new(v.x, v.y, v.z * k) + back * (v.z.max(0.0) * (1.0 - k))
    })
}

/// Ухо не уходит под землю: то, что ниже шарнира, ложится на неё назад, не теряя площади.
fn grounded(ear: Ear, back: Vec3) -> Ear {
    let reach = ear.axes[0].z.hypot(ear.axes[1].z);
    let below = ear.root.z - ear.center.z + reach;
    if below <= ear.root.z || below <= 0.0 {
        return ear;
    }
    let k = (ear.root.z / below).max(0.0);
    // Назад поперёк лопуха: иначе ухо сложится в черту
    let out = Vec3::new(ear.axes[0].x, ear.axes[0].y, 0.0).normalized();
    let back = (back - out * back.dot(out)).normalized();
    flattened(ear, k, |v| {
        Vec3::new(v.x, v.y, v.z * k) + back * ((-v.z).max(0.0) * (1.0 - k))
    })
}

/// Ухо, сжатое по высоте до доли `k` отображением `flat` от шарнира.
fn flattened(ear: Ear, k: f32, flat: impl Fn(Vec3) -> Vec3) -> Ear {
    let n = ear.inner;
    Ear {
        root: ear.root,
        center: ear.root + flat(ear.center - ear.root),
        axes: ear.axes.map(&flat),
        // Чем площе, тем больше изнанкой вниз: лежит серым вверх
        inner: Vec3::new(n.x * k, n.y * k, n.z)
            .normalized()
            .lerp(Vec3::new(0.0, 0.0, -1.0), 1.0 - k)
            .normalized(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: EarShape = EarShape {
        root: [0.7, -0.3, 0.3],
        radii: [0.6, 0.8],
        open: [1.95, 1.45],
        slump: 0.5,
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
    fn inner_side_faces_forward_and_spread_ear_sticks_out() {
        let head = Frame::new(0.0, 0.0, 0.0);
        // Голова высоко: земля уши не сплющивает
        let at = Vec3::new(0.0, 0.0, 50.0);
        let place = |open, tuck| settle(open, 0.0, 0.0).place(&SHAPE, &head, at, 10.0, tuck)[0];
        let (calm, open) = (place(SHAPE.open[0], 0.0), place(SHAPE.open[1], 0.0));
        // Левое ухо: наружу — против `right`
        let out = |ear: &Ear| -ear.center.dot(head.right);
        assert!(out(&open) > out(&calm) + 1.0);
        for ear in [calm, open] {
            assert!(ear.inner.dot(head.forward) > 0.7, "изнанкой вперёд {ear:?}");
        }
        assert!(
            calm.inner.dot(head.right) > 0.2,
            "в покое — чашей к морде, наружной стороной вбок"
        );
        assert!(calm.axes[0].dot(calm.inner).abs() < 1e-4);
        let asleep = place(SHAPE.open[0], 1.0);
        assert!(asleep.root.z < calm.root.z - 2.0, "во сне обвисают");
    }

    #[test]
    fn ears_lag_behind_turns_and_starts() {
        let want = SHAPE.open[0];
        let still = settle(want, 0.0, 0.0);
        let turning = settle(want, 3.0, 0.0);
        // Раскрытое ближе к прямому углу
        assert!(
            turning.open[1] < still.open[1] - 0.2,
            "внешнее к повороту раскрыто"
        );
        assert!(turning.open[0] > still.open[0] + 0.2);
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
