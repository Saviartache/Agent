//! Ноги. Лапа стоит на земле, пока не отстанет от места под телом дальше порога;
//! тогда она поднимается и переносится вперёд. Лапы ходят по очереди.

use super::Footfall;
use crate::animation::tween::Easing;
use crate::math::Vec2;

// Расстояния — design px, умножаются на масштаб; время — с; скорости — px/с.
/// Место лапы упреждается по скорости, но не дальше.
const LEAD_SECS: f32 = 0.10;
const LEAD_MAX: f32 = 9.0;
/// Отставание, с которого лапа шагает: на месте и на ходу.
const STRIDE_STILL: f32 = 2.6;
const STRIDE_MOVE: f32 = 6.5;
/// Скорость, с которой порог становится «ходовым».
const STRIDE_SPEED: f32 = 50.0;
/// Лапа вне очереди шагает, если отстала во столько раз сильнее порога.
const OUT_OF_TURN: f32 = 1.8;
const STEP_SECS_STILL: f32 = 0.22;
const STEP_SECS_MIN: f32 = 0.09;
/// Скорость, на которой шаг вдвое короче по времени.
const STEP_SPEED: f32 = 110.0;
const STEP_HEIGHT: f32 = 3.5;
const STEP_HEIGHT_FAST: f32 = 2.5;

/// Что нужно ногам от тела на этом шаге.
#[derive(Clone, Copy, Debug)]
pub struct Stance {
    /// Точка на земле между лапами пары.
    pub pos: Vec2,
    pub forward: Vec2,
    /// Единичный вектор вправо по ходу.
    pub right: Vec2,
    pub vel: Vec2,
    pub scale: f32,
    /// Разнос лап от оси тела, px.
    pub side: f32,
    pub grounded: bool,
    /// Доля обычного шага: длина, высота и время.
    pub stride: f32,
}

impl Stance {
    pub fn new(pos: Vec2, yaw: f32, vel: Vec2, scale: f32, side: f32, grounded: bool) -> Self {
        let forward = Vec2::from_angle(yaw);
        Self {
            pos,
            forward,
            right: Vec2::new(-forward.y, forward.x),
            vel,
            scale,
            side,
            grounded,
            stride: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Swing {
    from: Vec2,
    t: f32,
    secs: f32,
}

#[derive(Clone, Copy, Debug)]
struct Leg {
    at: Vec2,
    z: f32,
    swing: Option<Swing>,
}

#[derive(Clone, Debug)]
pub struct Feet {
    /// Левая и правая по ходу.
    legs: [Leg; 2],
    next: usize,
    airborne: bool,
}

impl Feet {
    /// `lead` — лапа, которая шагнёт первой.
    pub fn planted(stance: &Stance, lead: usize) -> Self {
        let leg = |i| Leg {
            at: ideal(i, stance, stance.pos),
            z: 0.0,
            swing: None,
        };
        Self {
            legs: [leg(0), leg(1)],
            next: lead,
            airborne: !stance.grounded,
        }
    }

    pub fn ground(&self, leg: usize) -> Vec2 {
        self.legs[leg].at
    }

    /// Высота лапы над землёй при шаге.
    pub fn lift(&self, leg: usize) -> f32 {
        self.legs[leg].z
    }

    pub fn swinging(&self, leg: usize) -> bool {
        self.legs[leg].swing.is_some()
    }

    pub fn update(&mut self, dt: f32, stance: &Stance, footfalls: &mut Vec<Footfall>) {
        if !stance.grounded {
            // В руках лапы не шагают
            self.airborne = true;
            self.legs = self.legs.map(|leg| Leg {
                swing: None,
                z: 0.0,
                ..leg
            });
            return;
        }
        if std::mem::take(&mut self.airborne) {
            *self = Self::planted(stance, self.next);
            return;
        }
        for index in 0..2 {
            self.advance(index, dt, stance, footfalls);
        }
        if !self.swinging(0) && !self.swinging(1) {
            self.begin(stance);
        }
    }

    fn advance(&mut self, index: usize, dt: f32, stance: &Stance, footfalls: &mut Vec<Footfall>) {
        let Some(mut swing) = self.legs[index].swing else {
            return;
        };
        swing.t += dt / swing.secs;
        let left = (swing.secs * (1.0 - swing.t)).max(0.0);
        let target = ideal(index, stance, stance.pos + stance.vel * left);
        if swing.t >= 1.0 {
            self.legs[index] = Leg {
                at: target,
                z: 0.0,
                swing: None,
            };
            self.next = 1 - index;
            footfalls.push(Footfall {
                pos: target,
                heading: stance.forward,
                left: index == 0,
            });
            return;
        }
        let height = height(stance);
        self.legs[index] = Leg {
            at: swing.from.lerp(target, Easing::InOutCubic.apply(swing.t)),
            z: (swing.t * std::f32::consts::PI).sin() * height,
            swing: Some(swing),
        };
    }

    fn begin(&mut self, stance: &Stance) {
        let lag = |feet: &Self, i: usize| feet.legs[i].at.distance(ideal(i, stance, stance.pos));
        let limit = stride(stance);
        let (first, second) = (self.next, 1 - self.next);
        let step = if lag(self, first) > limit {
            first
        } else if lag(self, second) > limit * OUT_OF_TURN {
            second
        } else {
            return;
        };
        let secs = (STEP_SECS_STILL / (1.0 + speed(stance) / STEP_SPEED)).max(STEP_SECS_MIN)
            * stance.stride;
        let leg = &mut self.legs[step];
        leg.swing = Some(Swing {
            from: leg.at,
            t: 0.0,
            secs,
        });
    }
}

/// Скорость в design px/с.
fn speed(stance: &Stance) -> f32 {
    stance.vel.length() / stance.scale
}

fn stride(stance: &Stance) -> f32 {
    let walking = (speed(stance) / STRIDE_SPEED).min(1.0);
    (STRIDE_STILL + (STRIDE_MOVE - STRIDE_STILL) * walking) * stance.scale * stance.stride
}

fn height(stance: &Stance) -> f32 {
    let fast = (speed(stance) / STEP_SPEED).min(1.0);
    (STEP_HEIGHT + STEP_HEIGHT_FAST * fast) * stance.scale * stance.stride
}

/// Место лапы пары, стоящей в `pos`: сбоку от оси и чуть впереди по ходу.
fn ideal(leg: usize, stance: &Stance, pos: Vec2) -> Vec2 {
    let side = if leg == 0 { -stance.side } else { stance.side };
    let lead = (stance.vel * LEAD_SECS).clamp_length(LEAD_MAX * stance.scale);
    pos + stance.right * side + lead
}
