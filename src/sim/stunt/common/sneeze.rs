//! Чих: два-три вдоха носом вверх и «апчхи» с отдачей назад. Грязная свинка
//! стряхивает всю грязь: брызги и лужицы вокруг.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, время — с.
const INHALE_SECS: f32 = 0.4;
const RECOIL_SPEED: f32 = 120.0;
const RECOIL_FRICTION: f32 = 8.0;
/// После чиха: встряска головой и пауза.
const AFTER_SECS: f32 = 0.7;
const SHAKE_HZ: f32 = 9.0;
const MUDDY: f32 = 0.2;
const SPLASH_RADIUS: f32 = 10.0;
const SPLASH_TTL: f32 = 6.0;
const SPLASH_RING: [f32; 2] = [25.0, 60.0];

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Sneeze {
    t: f32,
    inhales: u32,
    face: Vec2,
    done: bool,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_sneeze(&mut self) -> Option<Sneeze> {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(Sneeze {
            t: 0.0,
            inhales: 2 + (self.rng.chance(0.5) as u32),
            face: self.body.heading,
            done: false,
        })
    }

    pub(in crate::sim::stunt) fn step_sneeze(
        &mut self,
        mut act: Sneeze,
        dt: f32,
    ) -> Progress<Sneeze> {
        act.t += dt;
        let burst_at = act.inhales as f32 * INHALE_SECS;
        if act.t < burst_at {
            self.sneeze_inhale(act.t, act.inhales);
            return Progress::Going(act);
        }
        if !act.done {
            act.done = true;
            self.sneeze_burst(act.face);
        }
        let since = act.t - burst_at;
        if since >= AFTER_SECS {
            self.body.heading = act.face;
            self.halt();
            return Progress::Finished;
        }
        let decay = 1.0 - since / AFTER_SECS;
        let snap = (-since * 14.0).exp();
        self.antic.head_pitch = -0.6 * snap;
        self.antic.mouth = snap;
        self.antic.lids = snap;
        self.antic.head_yaw = 0.5 * decay * (TAU * SHAKE_HZ * since).sin();
        self.antic.head_roll = 0.3 * decay * (TAU * SHAKE_HZ * since).cos();
        self.keep_facing_driven(act.face);
        Progress::Going(act)
    }

    /// Каждый вдох глубже предыдущего: нос выше, рот шире, тело раздуто.
    fn sneeze_inhale(&mut self, t: f32, inhales: u32) {
        let p = (t / INHALE_SECS).fract();
        let depth = (t / INHALE_SECS).floor() + 1.0;
        let swell = (PI * p).sin();
        let height = (depth / inhales as f32).min(1.0);
        self.antic.pitch = 0.2 * height * swell;
        self.antic.head_pitch = 0.5 * height * swell;
        self.antic.neck_up = 3.0 * height * swell;
        self.antic.mouth = 0.6 * swell;
        self.antic.squash.y = 1.0 + 0.08 * swell;
    }

    fn sneeze_burst(&mut self, face: Vec2) {
        self.burst(Burst::Puff, self.head());
        self.drive = Some(Drive::new(-face * (RECOIL_SPEED * self.ui)).friction(RECOIL_FRICTION));
        if self.cond.muddy <= MUDDY {
            return;
        }
        self.cond.muddy = 0.0;
        for _ in 0..3 {
            let spot = self.sneeze_splash_spot();
            self.burst(Burst::Mud, spot);
        }
        for _ in 0..2 + self.rng.chance(0.5) as usize {
            let spot = self.sneeze_splash_spot();
            let radius = SPLASH_RADIUS * self.ui;
            self.mud.pour(spot, radius, SPLASH_TTL, &mut self.rng);
        }
    }

    fn sneeze_splash_spot(&mut self) -> Vec2 {
        let direction = Vec2::from_angle(self.rng.range(0.0, TAU));
        let reach = self.rng.range(SPLASH_RING[0], SPLASH_RING[1]) * self.ui;
        let spot = self.body.pos + direction * reach;
        self.bounds.clamp(spot, self.edge_margin())
    }
}
