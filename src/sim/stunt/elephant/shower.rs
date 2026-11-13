//! Душ: идёт к луже воды, набирает её хоботом, задирает хобот и поливает себе спину;
//! в конце отряхивается, хлопая ушами.

use std::f32::consts::TAU;

use super::super::Progress;
use super::TRUNK_REACH;
use crate::behavior::Directive;
use crate::effects::mud::Liquid;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px (design px — у тела), время — с.
const PUDDLE_RADIUS: f32 = 26.0;
const PUDDLE_TTL: f32 = 18.0;
const PUDDLE_DISTANCE: [f32; 2] = [150.0, 300.0];
const PUDDLE_MIN_DISTANCE: f32 = 90.0;
/// Допуск места: не теснее, чем приходит ходьба.
const REACH_SLACK: f32 = 9.0;
const WALK_SECS: f32 = 12.0;
const SUCK_SECS: f32 = 1.4;
/// Хлюпает кончиком хобота в луже.
const SIP_GAP: f32 = 0.35;
const RAISE_SECS: f32 = 0.4;
const SPRAY_SECS: f32 = 2.4;
const SPRAY_GAP: f32 = 0.18;
/// Хобот загнут над спиной; струя падает на спину, design px от точки под телом.
const SPRAY_PITCH: f32 = 1.25;
const SPRAY_BACK: f32 = 4.0;
const SPRAY_UP: f32 = 30.0;
const SHAKE_SECS: f32 = 1.0;
const SHAKE_HZ: f32 = 10.0;
const SPLASHES: usize = 3;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk(f32),
    Suck { face: Vec2, t: f32, sip: f32 },
    Spray { t: f32, gap: f32 },
    Shake(f32),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Shower {
    puddle: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_shower(&mut self) -> Option<Shower> {
        let puddle = self.spot_away(PUDDLE_DISTANCE, PUDDLE_MIN_DISTANCE, PUDDLE_RADIUS)?;
        self.mud.pour_liquid(
            puddle,
            PUDDLE_RADIUS * self.ui,
            PUDDLE_TTL,
            Liquid::Water,
            &mut self.rng,
        );
        let (_, spot) = self.reach_for(puddle, TRUNK_REACH);
        self.brain.push(Directive::MoveTo(spot));
        Some(Shower {
            puddle,
            phase: Phase::Walk(WALK_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_shower(&mut self, act: Shower, dt: f32) -> Progress<Shower> {
        let phase = match act.phase {
            Phase::Walk(left) => {
                let (face, spot) = self.reach_for(act.puddle, TRUNK_REACH);
                if self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                    self.halt();
                    Phase::Suck {
                        face,
                        t: 0.0,
                        sip: 0.0,
                    }
                } else if left <= dt {
                    self.mud.dry_at(act.puddle);
                    return Progress::Dropped;
                } else {
                    Phase::Walk(left - dt)
                }
            }
            Phase::Suck { face, t, sip } => {
                let t = t + dt;
                self.body.heading = face;
                self.trunk_down((t / RAISE_SECS).min(1.0));
                let sip = if sip <= dt {
                    self.burst(Burst::Water, self.muzzle_ground());
                    SIP_GAP
                } else {
                    sip - dt
                };
                if t >= SUCK_SECS {
                    Phase::Spray { t: 0.0, gap: 0.0 }
                } else {
                    Phase::Suck { face, t, sip }
                }
            }
            Phase::Spray { t, gap } => {
                let t = t + dt;
                let up = (t / RAISE_SECS).min(1.0);
                self.antic.head_pitch = SPRAY_PITCH * up;
                self.antic.neck_up = 3.0 * up;
                self.antic.wobble = 0.5 * (t * TAU * 3.0).sin();
                let gap = if up >= 1.0 && gap <= dt {
                    self.burst(Burst::Water, self.back_spray());
                    SPRAY_GAP
                } else {
                    gap - dt
                };
                if t >= SPRAY_SECS {
                    self.mud.dry_at(act.puddle);
                    for i in 0..SPLASHES {
                        let side = Vec2::from_angle(i as f32 * TAU / SPLASHES as f32);
                        self.burst(Burst::Water, self.body.pos + side * (16.0 * self.sprite));
                    }
                    Phase::Shake(0.0)
                } else {
                    Phase::Spray { t, gap }
                }
            }
            Phase::Shake(t) => {
                let t = t + dt;
                if t >= SHAKE_SECS {
                    return Progress::Finished;
                }
                let fade = 1.0 - t / SHAKE_SECS;
                self.antic.wobble = 1.8 * fade * (t * TAU * SHAKE_HZ).sin();
                self.antic.wings = fade;
                self.antic.flap = (t * SHAKE_HZ * 0.5).fract();
                Phase::Shake(t)
            }
        };
        Progress::Going(Shower { phase, ..act })
    }

    /// Куда падает струя: над спиной.
    fn back_spray(&self) -> Vec2 {
        let back = self.body.heading.normalized() * (-SPRAY_BACK * self.sprite);
        self.body.pos + back - Vec2::new(0.0, SPRAY_UP * self.sprite)
    }
}
