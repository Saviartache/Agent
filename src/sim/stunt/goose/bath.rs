//! Купание: плещется в луже воды, выходит и отряхивается веером брызг.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::mud::Liquid;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const PUDDLE_RADIUS: f32 = 34.0;
const PUDDLE_TTL: f32 = 16.0;
const PUDDLE_DISTANCE: [f32; 2] = [200.0, 400.0];
const PUDDLE_MIN_DISTANCE: f32 = 120.0;
/// Не добежал за это время — сценка сорвалась.
const RUN_SECS: f32 = 12.0;
/// Ближе этой доли радиуса к середине — уже в луже.
const ENTER: f32 = 0.6;
const SPLASH_SECS: f32 = 2.0;
const SPLASH_GAP: f32 = 0.3;
/// Выходит на столько от середины лужи.
const EXIT_DISTANCE: f32 = 48.0;
const EXIT_REACH: f32 = 8.0;
const EXIT_SECS: f32 = 4.0;
const SHAKE_SECS: f32 = 1.0;
const SHAKE_HZ: f32 = 12.0;
const SPRAYS: usize = 3;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run(f32),
    Splash { t: f32, gap: f32 },
    Exit(f32),
    Shake(f32),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Bath {
    puddle: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_bath(&mut self) -> Option<Bath> {
        let puddle = self.spot_away(PUDDLE_DISTANCE, PUDDLE_MIN_DISTANCE, PUDDLE_RADIUS)?;
        self.mud.pour_liquid(
            puddle,
            PUDDLE_RADIUS * self.ui,
            PUDDLE_TTL,
            Liquid::Water,
            &mut self.rng,
        );
        self.brain.push(Directive::RunTo(puddle));
        Some(Bath {
            puddle,
            phase: Phase::Run(RUN_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_bath(&mut self, act: Bath, dt: f32) -> Progress<Bath> {
        let phase = match act.phase {
            Phase::Run(_)
                if self.body.pos.distance(act.puddle) <= ENTER * PUDDLE_RADIUS * self.ui =>
            {
                self.halt();
                self.burst(Burst::Water, self.body.pos);
                Phase::Splash {
                    t: 0.0,
                    gap: SPLASH_GAP,
                }
            }
            Phase::Run(left) if left > dt => Phase::Run(left - dt),
            Phase::Run(_) => return Progress::Dropped,
            Phase::Splash { t, gap } => {
                let t = t + dt;
                if t >= SPLASH_SECS {
                    self.mud.dry_at(act.puddle);
                    let away = (self.body.pos - act.puddle).normalized();
                    let away = if away == Vec2::ZERO {
                        Vec2::new(1.0, 0.0)
                    } else {
                        away
                    };
                    let exit = act.puddle + away * (EXIT_DISTANCE * self.ui);
                    self.brain.push(Directive::MoveTo(
                        self.bounds.clamp(exit, self.edge_margin()),
                    ));
                    Phase::Exit(EXIT_SECS)
                } else {
                    self.splash_pose(t);
                    let gap = if gap <= dt {
                        self.burst(Burst::Water, self.body.pos);
                        SPLASH_GAP
                    } else {
                        gap - dt
                    };
                    Phase::Splash { t, gap }
                }
            }
            Phase::Exit(left) => {
                let outside =
                    self.body.pos.distance(act.puddle) >= (EXIT_DISTANCE - EXIT_REACH) * self.ui;
                if outside || left <= dt {
                    self.halt();
                    for i in 0..SPRAYS {
                        let side = Vec2::from_angle(i as f32 * TAU / SPRAYS as f32);
                        self.burst(Burst::Water, self.body.pos + side * (14.0 * self.sprite));
                    }
                    Phase::Shake(0.0)
                } else {
                    Phase::Exit(left - dt)
                }
            }
            Phase::Shake(t) => {
                let t = t + dt;
                if t >= SHAKE_SECS {
                    return Progress::Finished;
                }
                let fade = 1.0 - t / SHAKE_SECS;
                self.antic.wobble = 1.8 * fade * (t * TAU * SHAKE_HZ).sin();
                Phase::Shake(t)
            }
        };
        Progress::Going(Bath { phase, ..act })
    }

    /// Плещется: присел, машет крылышками, покачивается.
    fn splash_pose(&mut self, t: f32) {
        self.antic.crouch = 3.0;
        self.antic.wings = 0.45 + 0.25 * (t * TAU * 2.5).sin();
        self.antic.flap = (t * 3.0).fract();
        self.antic.wobble = 0.8 * (t * TAU * 3.0).sin();
    }
}
