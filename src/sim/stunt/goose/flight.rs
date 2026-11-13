//! Полёт: разбег с машущими крыльями и низкий перелёт; бывает, не взлетает или бьётся
//! головой о верх экрана.

use std::f32::consts::PI;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, скорость — px/с, высота — design px, время — с.
const MIN_ROOM: f32 = 250.0;
const RUN_SECS: f32 = 0.8;
const FLAP_HZ: f32 = 4.0;
const STRAIN_HZ: f32 = 7.0;
const FAIL_CHANCE: f32 = 0.25;
const CEILING_CHANCE: f32 = 0.2;
const CRUISE_SPEED: f32 = 300.0;
const CRUISE_LIFT: f32 = 60.0;
const FLY_SECS: f32 = 2.4;
const LAND_SECS: f32 = 0.5;
const LAND_FRICTION: f32 = 4.0;
/// Слабый подскок неудачника.
const HOP_LIFT: f32 = 2.0;
const HOP_SECS: f32 = 0.6;
const HOPS: u8 = 2;
const WALK_AWAY: f32 = 150.0;
const WALK_LIMIT: f32 = 3.0;
const CLIMB_SPEED: f32 = 250.0;
const CLIMB_DRIFT: f32 = 100.0;
const CLIMB_LIFT: f32 = 40.0;
const CLIMB_LIMIT: f32 = 6.0;
const FALL_SECS: f32 = 0.35;
const BOUNCE_LIFT: f32 = 6.0;
const BOUNCE_SECS: f32 = 0.25;
const DAZE_SECS: f32 = 1.0;
const HIT_DAZE: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Soar,
    Flop,
    Ceiling,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Run,
    Fly,
    Land,
    Hop(u8),
    Walk,
    Climb,
    Fall,
    Daze,
}

#[derive(Debug)]
pub(in crate::sim) struct Flight {
    kind: Kind,
    /// Куда летит по горизонтали: −1 влево, 1 вправо.
    side: f32,
    phase: Phase,
    /// Время в фазе и от начала.
    t: f32,
    total: f32,
    /// Высота в момент удара о верх.
    hit_lift: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_flight(&mut self) -> Option<Flight> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin();
        let (left, right) = (
            self.body.pos.x - area.min.x - margin,
            area.max.x - self.body.pos.x - margin,
        );
        if left.max(right) < MIN_ROOM * self.ui {
            return None;
        }
        let side = if left > right { -1.0 } else { 1.0 };
        let roll = self.rng.unit();
        let kind = if roll < FAIL_CHANCE {
            Kind::Flop
        } else if roll < FAIL_CHANCE + CEILING_CHANCE {
            Kind::Ceiling
        } else {
            Kind::Soar
        };
        let ahead = self.body.pos + Vec2::new(side * MIN_ROOM * self.ui, 0.0);
        self.brain.push(Directive::RunTo(ahead));
        Some(Flight {
            kind,
            side,
            phase: Phase::Run,
            t: 0.0,
            total: 0.0,
            hit_lift: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_flight(
        &mut self,
        mut act: Flight,
        dt: f32,
    ) -> Progress<Flight> {
        act.t += dt;
        act.total += dt;
        let ui = self.ui;
        match act.phase {
            Phase::Run => {
                self.flap(&act, FLAP_HZ);
                if act.t >= RUN_SECS {
                    let next = match act.kind {
                        Kind::Soar => {
                            let vel = Vec2::new(act.side * CRUISE_SPEED * ui, 0.0);
                            self.drive = Some(Drive::new(vel));
                            Phase::Fly
                        }
                        Kind::Flop => {
                            self.drive = Some(Drive::new(Vec2::ZERO));
                            self.halt();
                            Phase::Hop(0)
                        }
                        Kind::Ceiling => Phase::Climb,
                    };
                    enter(&mut act, next);
                }
            }
            Phase::Fly => {
                self.flap(&act, FLAP_HZ);
                self.antic.lift = CRUISE_LIFT * (PI * act.t / FLY_SECS).sin().max(0.0);
                if act.t >= FLY_SECS {
                    let vel = Vec2::new(act.side * CRUISE_SPEED * ui, 0.0);
                    self.drive = Some(Drive::new(vel).friction(LAND_FRICTION));
                    enter(&mut act, Phase::Land);
                }
            }
            Phase::Land => {
                self.antic.wings = 1.0 - act.t / LAND_SECS;
                self.antic.crouch = 0.3;
                if act.t >= LAND_SECS {
                    return self.end_flight();
                }
            }
            Phase::Hop(n) => {
                self.flap(&act, STRAIN_HZ);
                self.antic.lift = HOP_LIFT * (PI * act.t / HOP_SECS).sin().max(0.0);
                self.antic.neck_forward = 3.0;
                if act.t >= HOP_SECS {
                    if n + 1 < HOPS {
                        enter(&mut act, Phase::Hop(n + 1));
                    } else {
                        self.drive = None;
                        let away = self.body.pos + Vec2::new(act.side * WALK_AWAY * ui, 0.0);
                        self.brain.push(Directive::MoveTo(away));
                        enter(&mut act, Phase::Walk);
                    }
                }
            }
            Phase::Walk => {
                self.antic.lids = 0.3;
                if act.t >= WALK_LIMIT || self.arrived {
                    return self.end_flight();
                }
            }
            Phase::Climb => {
                self.flap(&act, FLAP_HZ);
                let vel = Vec2::new(act.side * CLIMB_DRIFT * ui, -CLIMB_SPEED * ui);
                self.drive = Some(Drive::new(vel));
                self.antic.lift = CLIMB_LIFT * (act.t / 1.0).min(1.0);
                self.antic.pitch = 0.3;
                if self.bumped.is_some_and(|n| n.y > 0.0) {
                    self.knock(&mut act);
                } else if act.t >= CLIMB_LIMIT {
                    return self.end_flight();
                }
            }
            Phase::Fall => {
                self.antic.eyes = Eyes::Wide;
                self.antic.lift = if act.t < FALL_SECS {
                    act.hit_lift * (1.0 - act.t / FALL_SECS).powi(2)
                } else {
                    let k = (act.t - FALL_SECS) / BOUNCE_SECS;
                    BOUNCE_LIFT * (PI * k).sin().max(0.0)
                };
                if act.t >= FALL_SECS + BOUNCE_SECS {
                    enter(&mut act, Phase::Daze);
                }
            }
            Phase::Daze => {
                self.antic.crouch = 0.4;
                if act.t >= DAZE_SECS {
                    return self.end_flight();
                }
            }
        }
        Progress::Going(act)
    }

    fn flap(&mut self, act: &Flight, hz: f32) {
        self.antic.wings = 1.0;
        self.antic.flap = (act.total * hz).fract();
    }

    /// Головой о верх экрана: перья, звёзды, падает.
    fn knock(&mut self, act: &mut Flight) {
        act.hit_lift = self.antic.lift;
        self.drive = Some(Drive::new(Vec2::ZERO));
        self.halt();
        let head = self.head();
        self.burst(Burst::Feather, head);
        self.burst(Burst::Stars, head);
        for _ in 0..2 {
            self.shed_feather(Vec2::new(-act.side, 0.0));
        }
        self.cond.dazed = HIT_DAZE;
        enter(act, Phase::Fall);
    }

    fn end_flight(&mut self) -> Progress<Flight> {
        self.drive = None;
        self.halt();
        Progress::Finished
    }
}

fn enter(act: &mut Flight, phase: Phase) {
    act.phase = phase;
    act.t = 0.0;
}
