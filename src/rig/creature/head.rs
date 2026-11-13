//! Голова на пружине: отстаёт и чуть перелетает, когда тело трогается и встаёт.
//! Взгляд поворачивается к цели, глаза моргают, иногда голова косится в сторону.

use std::f32::consts::{PI, TAU};

use crate::math::Vec2;
use crate::rig::vec3::Vec3;
use crate::rng::Rng;

/// Частота пружины, рад/с, и затухание (меньше 1 — перелёт).
const SPRING_OMEGA: f32 = 15.0;
const SPRING_DAMPING: f32 = 0.55;
/// Доля ускорения тела, которую голова чувствует как инерцию.
const INERTIA: f32 = 2.2;
/// Дальше этого от цели голова не уходит, design px.
const MAX_LAG: f32 = 14.0;
const YAW_RATE: f32 = 12.0;
const GLANCE_GAP_SECS: [f32; 2] = [1.2, 3.5];
const GLANCE_CHANCE: f32 = 0.55;
const GLANCE_ANGLE: f32 = 0.8;
const BLINK_GAP_SECS: [f32; 2] = [1.8, 5.5];
const BLINK_SECS: f32 = 0.16;

/// Что нужно голове от тела на этом шаге.
#[derive(Clone, Copy, Debug)]
pub struct Aim {
    /// Основание шеи и желаемое положение головы от него (мир).
    pub base: Vec3,
    pub rest: Vec3,
    /// Ускорение тела по земле, px/с².
    pub accel: Vec2,
    pub body_yaw: f32,
    /// Куда повёрнуто тело по ходу, если смотреть некуда.
    pub heading_yaw: f32,
    pub look_at: Option<Vec2>,
    /// Добавка жеста к повороту головы, рад.
    pub yaw_offset: f32,
    /// 1 — стоит спокойно, 0 — бежит: на бегу не косится.
    pub calm: f32,
    pub scale: f32,
    /// Гибкость шеи: доля инерции и отставания головы.
    pub slack: f32,
    /// Предел поворота головы от оси тела, рад.
    pub yaw_limit: f32,
}

#[derive(Clone, Debug)]
pub struct HeadState {
    /// Голова относительно основания шеи.
    rel: Vec3,
    vel: Vec3,
    yaw: f32,
    glance: f32,
    glance_in: f32,
    blink_in: f32,
    /// 0..1 внутри моргания; `None` — глаза открыты.
    blink: Option<f32>,
}

pub fn wrap_angle(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped.is_finite() { wrapped } else { 0.0 }
}

pub fn yaw_of(direction: Vec2) -> f32 {
    direction.y.atan2(direction.x)
}

impl HeadState {
    pub fn resting(rest: Vec3, yaw: f32) -> Self {
        Self {
            rel: rest,
            vel: Vec3::ZERO,
            yaw,
            glance: 0.0,
            glance_in: GLANCE_GAP_SECS[0],
            blink_in: BLINK_GAP_SECS[0],
            blink: None,
        }
    }

    pub fn center(&self, base: Vec3) -> Vec3 {
        base + self.rel
    }

    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// 1 — глаза открыты, 0 — моргнули.
    pub fn eyes_open(&self) -> f32 {
        1.0 - self.blink.map_or(0.0, |phase| (phase * PI).sin())
    }

    pub fn update(&mut self, dt: f32, aim: &Aim, rng: &mut Rng) {
        self.spring(dt, aim);
        self.turn(dt, aim);
        self.tick_glance(dt, rng);
        self.tick_blink(dt, rng);
    }

    fn spring(&mut self, dt: f32, aim: &Aim) {
        let inertia = Vec3::on_ground(aim.accel, 0.0) * (-INERTIA * aim.slack);
        let pull = (aim.rest - self.rel) * (SPRING_OMEGA * SPRING_OMEGA);
        let drag = self.vel * (2.0 * SPRING_DAMPING * SPRING_OMEGA);
        self.vel = self.vel + (pull - drag + inertia) * dt;
        self.rel = self.rel + self.vel * dt;
        // Страховка от огромных шагов и мусора на входе
        let lag = self.rel - aim.rest;
        let limit = MAX_LAG * aim.scale * aim.slack;
        if !self.rel.is_finite() || lag.length() > limit {
            self.rel = if self.rel.is_finite() {
                aim.rest + lag.normalized() * limit
            } else {
                aim.rest
            };
            self.vel = Vec3::ZERO;
        }
    }

    fn turn(&mut self, dt: f32, aim: &Aim) {
        let want = match aim.look_at {
            Some(at) => yaw_of(at - self.center(aim.base).ground()),
            None => aim.heading_yaw + self.glance * aim.calm,
        } + aim.yaw_offset;
        let limit = aim.yaw_limit;
        let limited = aim.body_yaw + wrap_angle(want - aim.body_yaw).clamp(-limit, limit);
        self.yaw += wrap_angle(limited - self.yaw) * (1.0 - (-dt * YAW_RATE).exp());
        self.yaw = aim.body_yaw + wrap_angle(self.yaw - aim.body_yaw).clamp(-limit, limit);
    }

    fn tick_glance(&mut self, dt: f32, rng: &mut Rng) {
        self.glance_in -= dt;
        if self.glance_in > 0.0 {
            return;
        }
        self.glance_in = rng.range(GLANCE_GAP_SECS[0], GLANCE_GAP_SECS[1]);
        self.glance = if rng.chance(GLANCE_CHANCE) {
            rng.range(-GLANCE_ANGLE, GLANCE_ANGLE)
        } else {
            0.0
        };
    }

    fn tick_blink(&mut self, dt: f32, rng: &mut Rng) {
        match self.blink {
            Some(phase) if phase + dt / BLINK_SECS < 1.0 => {
                self.blink = Some(phase + dt / BLINK_SECS);
            }
            Some(_) => {
                self.blink = None;
                self.blink_in = rng.range(BLINK_GAP_SECS[0], BLINK_GAP_SECS[1]);
            }
            None => {
                self.blink_in -= dt;
                if self.blink_in <= 0.0 {
                    self.blink = Some(0.0);
                }
            }
        }
    }
}
