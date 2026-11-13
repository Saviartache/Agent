//! Ведро падает сверху на персонажа: тот сплющен и косоглаз, потом пружинит обратно,
//! а ведро отскакивает вбок и гаснет.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::{Drive, Sim};

/// Расстояния — логические px, время — с.
const DROP_HEIGHT: f32 = 300.0;
const BUCKET_TTL: f32 = 20.0;
/// Ведро не долетело — сценка сорвалась.
const FALL_TIMEOUT: f32 = 4.0;
const SQUASH: Vec2 = Vec2::new(1.6, 0.25);
const STUNNED_SECS: f32 = 1.5;
const SPRING_SECS: f32 = 0.9;
const SPRING_DECAY: f32 = 3.5;
const SPRING_HZ: f32 = 1.4;
const BUCKET_KICK: f32 = 220.0;
const BUCKET_HOP: f32 = 160.0;
const BUCKET_LIFE: f32 = 4.0;

#[derive(Debug)]
enum Phase {
    Fall(f32),
    Stunned(f32),
    Spring(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Flatten {
    bucket: PropId,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_flatten(&mut self) -> Option<Flatten> {
        self.drive = Some(Drive::new(Vec2::ZERO));
        let height = DROP_HEIGHT * self.ui;
        let bucket = self
            .props
            .spawn(PropKind::Bucket, self.body.pos, height, self.ui, BUCKET_TTL);
        Some(Flatten {
            bucket,
            phase: Phase::Fall(FALL_TIMEOUT),
        })
    }

    pub(in crate::sim::stunt) fn step_flatten(
        &mut self,
        mut act: Flatten,
        dt: f32,
    ) -> Progress<Flatten> {
        match act.phase {
            Phase::Fall(left) => {
                let Some(bucket) = self.props.get(act.bucket) else {
                    return Progress::Dropped;
                };
                if bucket.z <= 0.0 {
                    self.land(act.bucket);
                    act.phase = Phase::Stunned(STUNNED_SECS);
                } else if left > dt {
                    act.phase = Phase::Fall(left - dt);
                } else {
                    return Progress::Dropped;
                }
            }
            Phase::Stunned(left) => {
                self.antic.squash = SQUASH;
                self.antic.eyes = Eyes::Crossed;
                act.phase = if left > dt {
                    Phase::Stunned(left - dt)
                } else {
                    Phase::Spring(0.0)
                };
            }
            Phase::Spring(t) => {
                // Затухающие колебания: 0,25 → 1,15 → 0,95 → 1
                let wave = (-SPRING_DECAY * t).exp() * (TAU * SPRING_HZ * t).cos();
                let y = 1.0 - (1.0 - SQUASH.y) * wave;
                self.antic.squash = Vec2::new(1.0 + (1.0 - y) * 0.8, y);
                self.antic.eyes = Eyes::Crossed;
                if t + dt >= SPRING_SECS {
                    self.cond.dazed = 0.5;
                    return Progress::Finished;
                }
                act.phase = Phase::Spring(t + dt);
            }
        }
        Progress::Going(act)
    }

    /// Удар: ведро отскакивает вбок и гаснет.
    fn land(&mut self, bucket: PropId) {
        let side = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let (kick, hop) = (BUCKET_KICK * self.ui, BUCKET_HOP * self.ui);
        if let Some(bucket) = self.props.get_mut(bucket) {
            bucket.vel = Vec2::new(side * kick, 0.0);
            bucket.vz = hop;
            bucket.spin = side * 5.0;
            bucket.expire_in(BUCKET_LIFE);
        }
        self.burst(Burst::Dust, self.body.pos);
    }
}
