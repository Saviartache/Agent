//! Театральная смерть: шатается, хватается за грудь, падает на спину и лежит с высунутым
//! языком, пока курсор рядом. Курсор ушёл — вскакивает как ни в чём не бывало.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::animation::tween::Easing;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::{Drive, Sim};

/// Расстояния — логические px, время — с.
const TIMEOUT: f32 = 24.0;
/// Играет на зрителя, если курсор ближе.
const AUDIENCE: f32 = 200.0;
/// Лежит, пока курсор ближе.
const LIE_WHILE: f32 = 250.0;
const LIE_MAX: f32 = 12.0;
/// Без зрителя лежит недолго.
const LIE_ALONE: f32 = 3.0;
/// Курсор должен уйти так надолго, чтобы встать.
const GONE_SECS: f32 = 0.6;
const STAGGER_SECS: f32 = 1.4;
const FALL_SECS: f32 = 0.5;
const RISE_SECS: f32 = 0.35;
/// Подскок при вскакивании, design px.
const HOP: f32 = 26.0;
const CHEER_SECS: f32 = 1.0;

#[derive(Debug)]
enum Phase {
    Stagger(f32),
    Fall(f32),
    /// Сколько уже лежит и сколько курсор отсутствует.
    Lie {
        lying: f32,
        away: f32,
    },
    Rise(f32),
    Cheer(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct FakeDeath {
    phase: Phase,
    watched: bool,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_fake_death(&mut self) -> Option<FakeDeath> {
        let watched = self.near_cursor(AUDIENCE);
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(FakeDeath {
            phase: Phase::Stagger(STAGGER_SECS),
            watched,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_fake_death(
        &mut self,
        mut act: FakeDeath,
        dt: f32,
    ) -> Progress<FakeDeath> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        match act.phase {
            Phase::Stagger(left) => {
                let u = 1.0 - left / STAGGER_SECS;
                self.antic.wobble = 6.0 * u * (TAU * 2.5 * left).sin();
                self.antic.neck_up = (u * 4.0).min(1.0);
                self.antic.spin = PI * Easing::InOutCubic.apply((u - 0.3) / 0.7);
                act.phase = if left > dt {
                    Phase::Stagger(left - dt)
                } else {
                    Phase::Fall(0.0)
                };
            }
            Phase::Fall(t) => {
                self.antic.spin = PI;
                self.antic.roll = PI * Easing::InQuad.apply(t / FALL_SECS);
                self.antic.eyes = Eyes::Crossed;
                act.phase = if t + dt < FALL_SECS {
                    Phase::Fall(t + dt)
                } else {
                    self.burst(Burst::Dust, self.body.pos);
                    Phase::Lie {
                        lying: 0.0,
                        away: 0.0,
                    }
                };
            }
            Phase::Lie { lying, away } => {
                self.dead();
                let near = self.near_cursor(LIE_WHILE);
                let away = if near { 0.0 } else { away + dt };
                let lying = lying + dt;
                let up = if act.watched {
                    away >= GONE_SECS || lying >= LIE_MAX
                } else {
                    lying >= LIE_ALONE
                };
                act.phase = if up {
                    Phase::Rise(0.0)
                } else {
                    Phase::Lie { lying, away }
                };
            }
            Phase::Rise(t) => {
                let u = (t / RISE_SECS).min(1.0);
                self.antic.spin = PI * (1.0 - Easing::OutQuad.apply(u));
                self.antic.roll = PI * (1.0 - Easing::OutQuad.apply(u));
                self.antic.lift = HOP * (PI * u).sin();
                if t + dt >= RISE_SECS {
                    self.perform_quietly(self.character.actions().celebrate);
                    act.phase = Phase::Cheer(CHEER_SECS);
                } else {
                    act.phase = Phase::Rise(t + dt);
                }
            }
            Phase::Cheer(left) if left > dt => act.phase = Phase::Cheer(left - dt),
            Phase::Cheer(_) => return Progress::Finished,
        }
        Progress::Going(act)
    }

    /// Лежит на спине: крестики, язык, рот приоткрыт.
    fn dead(&mut self) {
        self.antic.spin = PI;
        self.antic.roll = PI;
        self.antic.eyes = Eyes::Crossed;
        self.antic.tongue = true;
        self.antic.mouth = 0.5;
    }

    fn near_cursor(&self, reach: f32) -> bool {
        self.cursor
            .is_some_and(|cursor| cursor.distance(self.body.pos) <= reach * self.ui)
    }
}
