//! Тобогган: разбег вперевалку, плюх на живот и скольжение с ластами назад;
//! за пузом остаётся полоса. Встаёт, где остановился.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, px/с, 1/с, с.
const RUN_DISTANCE: [f32; 2] = [300.0, 420.0];
const RUN_MIN: f32 = 200.0;
const RUN_SECS: f32 = 0.8;
const SLIDE_SPEED: f32 = 380.0;
const FRICTION: f32 = 1.3;
const STOPPED: f32 = 15.0;
/// Плюх и подъём, с.
const DROP_SECS: f32 = 0.2;
const RISE_SECS: f32 = 0.5;
const TIMEOUT: f32 = 10.0;
/// Голова поднята, ласты отведены назад.
const NECK_UP: f32 = 2.0;
const WINGS: f32 = 0.35;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run,
    /// `trail` — докуда дотянулась полоса.
    Slide {
        trail: Vec2,
    },
    Rise,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Toboggan {
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_toboggan(&mut self) -> Option<Toboggan> {
        let to = self.spot_away(RUN_DISTANCE, RUN_MIN, 40.0)?;
        self.brain.push(Directive::RunTo(to));
        Some(Toboggan {
            phase: Phase::Run,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_toboggan(
        &mut self,
        mut act: Toboggan,
        dt: f32,
    ) -> Progress<Toboggan> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            self.halt();
            return Progress::Dropped;
        }
        match act.phase {
            Phase::Run => {
                if act.t >= RUN_SECS {
                    let dir = match self.body.vel.normalized() {
                        v if v.length() > 0.0 => v,
                        _ => self.body.heading,
                    };
                    self.halt();
                    let vel = dir * (SLIDE_SPEED * self.ui);
                    self.drive = Some(Drive::new(vel).friction(FRICTION));
                    self.burst(Burst::Dust, self.body.pos);
                    act.phase = Phase::Slide {
                        trail: self.body.pos,
                    };
                    act.t = 0.0;
                }
            }
            Phase::Slide { trail } => {
                self.prone((act.t / DROP_SECS).min(1.0));
                self.antic.neck_up = NECK_UP;
                self.antic.wings = WINGS;
                let trail = self.stamp_between(trail, self.body.pos);
                act.phase = Phase::Slide { trail };
                if self.body.vel.length() < STOPPED * self.ui {
                    self.drive = None;
                    self.halt();
                    act.phase = Phase::Rise;
                    act.t = 0.0;
                }
            }
            Phase::Rise => {
                self.prone(1.0 - (act.t / RISE_SECS).min(1.0));
                if act.t >= RISE_SECS {
                    return Progress::Finished;
                }
            }
        }
        Progress::Going(act)
    }
}
