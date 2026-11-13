//! Накладка сценки на позу и вид тела. `Antic` задаёт сценка каждый шаг заново: чем поза
//! отличается от жеста прямо сейчас. `Look` — следы состояний на теле (грязь, проплешина),
//! живут дольше сценки. Величины — design px и радианы.

use crate::math::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eyes {
    #[default]
    Normal,
    /// Крестики: «умер», оглушён.
    Crossed,
    /// Распахнуты: испуг, удивление.
    Wide,
}

/// Поправки сценки поверх жеста; `Antic::default()` — ничего не меняет.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Antic {
    /// Тело над землёй: прыжок, полёт.
    pub lift: f32,
    /// Растяжение тела: `x` — вдоль и вбок, `y` — по высоте; `(1, 1)` — обычное.
    pub squash: Vec2,
    /// Поворот тела вокруг оси хода: π/2 — на боку, π — на спине.
    pub roll: f32,
    /// Добавка к наклону тела: плюс — нос вверх.
    pub pitch: f32,
    /// Добавка к повороту тела по земле: кружится на месте.
    pub spin: f32,
    pub neck_forward: f32,
    pub neck_up: f32,
    pub neck_side: f32,
    pub head_pitch: f32,
    pub head_roll: f32,
    pub head_yaw: f32,
    pub crouch: f32,
    /// Боковая тряска тела.
    pub wobble: f32,
    /// 0…1: веки опущены.
    pub lids: f32,
    /// 0…1: лапы поджаты.
    pub tuck: f32,
    pub eyes: Eyes,
    /// 0…1: клюв или рот открыт.
    pub mouth: f32,
    /// Язык набок.
    pub tongue: bool,
    /// Крылья гуся: 0 — сложены, 1 — раскрыты.
    pub wings: f32,
    /// Взмах крыльев, 0…1 — фаза; рисуется при `wings > 0`.
    pub flap: f32,
    /// 0…1: шея гуся завязана узлом.
    pub knot: f32,
}

impl Default for Antic {
    fn default() -> Self {
        Self {
            lift: 0.0,
            squash: Vec2::new(1.0, 1.0),
            roll: 0.0,
            pitch: 0.0,
            spin: 0.0,
            neck_forward: 0.0,
            neck_up: 0.0,
            neck_side: 0.0,
            head_pitch: 0.0,
            head_roll: 0.0,
            head_yaw: 0.0,
            crouch: 0.0,
            wobble: 0.0,
            lids: 0.0,
            tuck: 0.0,
            eyes: Eyes::Normal,
            mouth: 0.0,
            tongue: false,
            wings: 0.0,
            flap: 0.0,
            knot: 0.0,
        }
    }
}

impl Antic {
    /// Мусор на входе — без накладки.
    pub fn cleaned(self) -> Self {
        let fields = [
            self.lift,
            self.squash.x,
            self.squash.y,
            self.roll,
            self.pitch,
            self.spin,
            self.neck_forward,
            self.neck_up,
            self.neck_side,
            self.head_pitch,
            self.head_roll,
            self.head_yaw,
            self.crouch,
            self.wobble,
            self.lids,
            self.tuck,
            self.mouth,
            self.wings,
            self.flap,
            self.knot,
        ];
        if fields.iter().all(|v| v.is_finite()) && self.squash.x > 0.0 && self.squash.y > 0.0 {
            self
        } else {
            Self::default()
        }
    }
}

/// Что состояния оставили на теле, всё 0…1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// Грязь на спине и копытах.
    pub mud: f32,
    /// Раздут после еды.
    pub bloat: f32,
    /// Грязный пятачок или клюв.
    pub snout: f32,
}

impl Look {
    pub fn cleaned(self) -> Self {
        let clamp = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Self {
            mud: clamp(self.mud),
            bloat: clamp(self.bloat),
            snout: clamp(self.snout),
        }
    }
}
