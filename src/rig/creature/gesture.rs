//! Жесты: чем поза отличается от спокойной стойки в данный момент клипа.
//! Все величины — design px и радианы; к миру их приводит сборка позы.

use std::f32::consts::{PI, TAU};

use super::antic::Antic;
use crate::animation::tween::Easing;
use crate::rig::Gesture;

/// Часы тела идут по кругу; частоты кратны 1/60 Гц, поэтому на стыке нет скачка.
pub const CLOCK_WRAP: f32 = 60.0;

/// Пределы накладки: выше холста тело не уходит, растяжение не безгранично.
const LIFT_LIMIT: f32 = 60.0;
const STRETCH_LIMIT: f32 = 3.0;
/// Бег (доля от полной скорости) вытягивает шею и наклоняет тело.
const RUN_NECK_FORWARD: f32 = 4.0;
const RUN_NECK_DOWN: f32 = -6.0;
const RUN_LEAN: f32 = -0.3;
const CALM_BREATH: f32 = 0.35;
const SLEEP_BREATH: f32 = 0.9;
/// Насколько приседает спящий, px.
const SLEEP_CROUCH: f32 = 5.5;

macro_rules! mods {
    ($($(#[$doc:meta])* $field:ident),* $(,)?) => {
        /// Поправки к стойке; нули — спокойная стойка.
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub struct Mods {
            $($(#[$doc])* pub $field: f32,)*
        }

        impl Mods {
            /// Сдвигает поправки к `to` на долю `k`.
            pub fn approach(&mut self, to: &Self, k: f32) {
                $(self.$field += (to.$field - self.$field) * k;)*
            }
        }
    };
}

mods! {
    /// Подъём всего тела над стойкой (прыжок).
    hop,
    crouch,
    /// Добавка к наклону тела.
    pitch,
    neck_forward,
    neck_up,
    neck_side,
    /// 0..1: голова лежит на спине.
    curl,
    head_pitch,
    head_roll,
    head_yaw,
    /// 0..1: веки опущены.
    lids,
    /// 0..1: лапы поджаты.
    tuck,
    /// Боковая тряска тела, px.
    wobble,
    /// Амплитуда дыхания, px.
    breath,
    /// Поворот тела вокруг оси хода, рад.
    roll,
    /// Растяжение тела минус единица: вдоль и вбок, по высоте.
    stretch_x,
    stretch_y,
    /// 0..1: размах виляния хвостом.
    wag,
}

impl Mods {
    /// Накладка сценки поверх жеста.
    pub fn overlay(&mut self, antic: &Antic) {
        self.hop += antic.lift.clamp(-LIFT_LIMIT, LIFT_LIMIT);
        self.roll += antic.roll;
        self.stretch_x += (antic.squash.x - 1.0).clamp(-1.0, STRETCH_LIMIT);
        self.stretch_y += (antic.squash.y - 1.0).clamp(-1.0, STRETCH_LIMIT);
        self.pitch += antic.pitch;
        self.neck_forward += antic.neck_forward;
        self.neck_up += antic.neck_up;
        self.neck_side += antic.neck_side;
        self.head_pitch += antic.head_pitch;
        self.head_roll += antic.head_roll;
        self.head_yaw += antic.head_yaw;
        self.crouch += antic.crouch;
        self.wobble += antic.wobble;
        self.lids = self.lids.max(antic.lids);
        self.tuck = self.tuck.max(antic.tuck);
        self.wag = self.wag.max(antic.wag);
    }
}

fn cycle(clock: f32, hz: f32) -> f32 {
    (TAU * hz * clock).sin()
}

/// Колокол `0 → 1 → 0` на отрезке `[from, to]` фазы.
fn pulse(p: f32, from: f32, to: f32) -> f32 {
    if p <= from || p >= to {
        return 0.0;
    }
    ((p - from) / (to - from) * PI).sin()
}

fn ramp(p: f32, from: f32, to: f32) -> f32 {
    Easing::InOutCubic.apply((p - from) / (to - from))
}

/// Плавно вошёл в начале и вышел в конце одноразового жеста.
fn envelope(p: f32) -> f32 {
    ramp(p, 0.0, 0.15) * (1.0 - ramp(p, 0.85, 1.0))
}

/// Размер от обычного: «выскакивание» при появлении и сжатие при уходе.
pub fn size(gesture: Gesture, progress: f32) -> f32 {
    match gesture {
        Gesture::Spawn => Easing::OutBack.apply(progress),
        Gesture::Despawn => 1.0 - Easing::InQuad.apply(progress),
        _ => 1.0,
    }
}

/// `run` — доля скорости бега (0..1); `progress` — фаза клипа.
pub fn target(gesture: Gesture, progress: f32, clock: f32, run: f32) -> Mods {
    let p = progress;
    let mut m = Mods {
        neck_forward: RUN_NECK_FORWARD * run,
        neck_up: RUN_NECK_DOWN * run,
        pitch: RUN_LEAN * run,
        breath: CALM_BREATH,
        ..Mods::default()
    };
    match gesture {
        Gesture::Idle => {}
        Gesture::Walk => m.neck_forward += 1.5,
        Gesture::Run => {
            m.neck_forward += 1.5;
            m.pitch -= 0.05;
        }
        Gesture::Look => {
            m.head_yaw = 0.95 * (TAU * p).sin();
            m.neck_up = 2.0 * pulse(p, 0.0, 1.0);
        }
        Gesture::React => {
            let honk = pulse(p, 0.15, 0.7);
            m.hop = 7.0 * pulse(p, 0.0, 0.45);
            m.neck_up = 3.0 * honk;
            m.head_pitch = 0.4 * honk;
            m.wag = envelope(p);
        }
        Gesture::Interact => peck(&mut m, pulse(p, 0.15, 0.75), 26.0, 12.0),
        Gesture::Celebrate => {
            let hop = (3.0 * PI * p).sin().abs();
            m.hop = 6.0 * hop;
            m.neck_up = 3.0 * hop;
            m.head_pitch = 0.3 * hop;
            m.wag = 1.0;
        }
        Gesture::Think => {
            m.head_roll = 0.38;
            m.head_yaw = 0.3;
            m.head_pitch = 0.15;
            m.neck_up = -2.0;
            m.neck_side = 2.5 * cycle(clock, 0.75);
        }
        Gesture::Busy => {
            // Склонился над делом: частые лёгкие тычки и медленный взгляд вдоль строки
            let tap = (3.0 * PI * p).sin().abs().powi(4);
            let scan = cycle(clock, 0.25);
            m.neck_forward += 6.0 + 2.0 * tap;
            m.neck_up -= 3.0 + 2.5 * tap;
            m.head_pitch = -0.3 - 0.2 * tap;
            m.head_yaw = 0.25 * scan;
            m.neck_side = 1.5 * scan;
        }
        Gesture::Warning => {
            let on = envelope(p);
            m.neck_up = 9.0 * on;
            m.neck_forward = -on;
            m.head_pitch = (0.15 + 0.2 * cycle(clock, 2.5)) * on;
        }
        Gesture::Error => {
            let shake = (p / 0.6).min(1.0);
            let droop = ramp(p, 0.5, 0.8) * (1.0 - ramp(p, 0.9, 1.0));
            m.head_yaw = 0.9 * (TAU * 2.5 * shake).sin() * (1.0 - shake);
            m.neck_up = -10.0 * droop;
            m.head_pitch = -0.6 * droop;
            m.crouch = 2.5 * droop;
            m.lids = 0.6 * droop;
        }
        Gesture::Sleep => sleeping(&mut m, 1.0),
        Gesture::WakeUp => {
            let rise = 1.0 - ramp(p, 0.0, 0.5);
            let shake = pulse(p, 0.45, 0.95);
            sleeping(&mut m, rise);
            m.wobble = 1.2 * cycle(clock, 5.0) * shake;
            m.head_yaw = 0.5 * cycle(clock, 5.0) * shake;
        }
        Gesture::Dragged => {
            m.neck_up = 4.0;
            m.head_pitch = 0.2;
            m.pitch = 0.1;
            m.wobble = 1.5 * cycle(clock, 3.0);
        }
        Gesture::Spawn => m.hop = 5.0 * pulse(p, 0.6, 1.0),
        Gesture::Despawn => m.crouch = 3.0 * p,
        Gesture::Wave => {
            // Кивает набок: крыльев у гуся нет
            let on = envelope(p);
            m.head_roll = 0.45 * cycle(clock, 2.5) * on;
            m.neck_side = 3.0 * cycle(clock, 2.5) * on;
            m.neck_up = 3.0 * on;
        }
    }
    m
}

/// Клевок: голова вперёд и вниз.
fn peck(m: &mut Mods, strike: f32, down: f32, forward: f32) {
    m.neck_forward += forward * strike;
    m.neck_up -= down * strike;
    m.head_pitch = -0.9 * strike;
    m.pitch -= 0.15 * strike;
    m.crouch = 1.5 * strike;
}

/// `depth` — глубина сна: 1 спит, 0 проснулся.
fn sleeping(m: &mut Mods, depth: f32) {
    m.curl = depth;
    m.crouch = SLEEP_CROUCH * depth;
    m.tuck = depth;
    m.lids = depth;
    m.pitch -= 0.2 * depth;
    m.head_pitch = -0.2 * depth;
    m.head_roll = 0.1 * depth;
    m.breath = CALM_BREATH + (SLEEP_BREATH - CALM_BREATH) * depth;
}
