//! Каток: разбег и пузом по экрану с вращением.

use std::f32::consts::{FRAC_PI_2, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, рад, с.
const RUN_DISTANCE: [f32; 2] = [300.0, 420.0];
const RUN_MIN: f32 = 200.0;
const RUN_SECS: f32 = 0.8;
const SLIDE_SPEED: f32 = 350.0;
const FRICTION: f32 = 1.2;
const STOPPED: f32 = 15.0;
const DUST_GAP: f32 = 0.15;
const LIE_SECS: f32 = 1.0;
const TIMEOUT: f32 = 10.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run,
    Slide { speed: f32, dust: f32 },
    Lie,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Slide {
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_slide(&mut self) -> Option<Slide> {
        let to = self.spot_away(RUN_DISTANCE, RUN_MIN, 40.0)?;
        self.brain.push(Directive::RunTo(to));
        Some(Slide {
            phase: Phase::Run,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_slide(&mut self, mut act: Slide, dt: f32) -> Progress<Slide> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            return Progress::Dropped;
        }
        let mut next = None;
        match act.phase {
            Phase::Run => {
                if act.t >= RUN_SECS {
                    let dir = match self.body.vel.normalized() {
                        v if v.length() > 0.0 => v,
                        _ => self.body.heading,
                    };
                    let speed = SLIDE_SPEED * self.ui;
                    self.halt();
                    self.drive = Some(Drive::new(dir * speed).friction(FRICTION));
                    next = Some(Phase::Slide { speed, dust: 0.0 });
                }
            }
            Phase::Slide { speed, dust } => {
                let now = self.body.vel.length();
                // Спин копит оборот по мере торможения
                self.belly();
                self.antic.spin = TAU * (1.0 - now / speed).clamp(0.0, 1.0);
                let dust = if dust <= 0.0 {
                    self.burst(Burst::Dust, self.body.pos);
                    DUST_GAP
                } else {
                    dust - dt
                };
                act.phase = Phase::Slide { speed, dust };
                if now < STOPPED * self.ui {
                    self.drive = None;
                    self.halt();
                    next = Some(Phase::Lie);
                }
            }
            Phase::Lie => {
                self.belly();
                self.antic.roll = FRAC_PI_2;
                if act.t >= LIE_SECS {
                    return Progress::Finished;
                }
            }
        }
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }

    fn belly(&mut self) {
        self.antic.crouch = 8.0;
        self.antic.tuck = 1.0;
    }
}
