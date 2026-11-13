//! Испугался себя: заметил свой след (или тень) позади, прыгнул и убежал; вернулся
//! обнюхать. Иногда ходит по кругу «за собой», понимает и смущается.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const SECS: f32 = 14.0;
const NOTICE_SECS: f32 = 0.6;
const HOP: f32 = 20.0;
const HOP_SECS: f32 = 0.4;
const FLEE_SPEED: f32 = 300.0;
const FLEE_FRICTION: f32 = 3.0;
/// Возвращается обнюхать через столько от начала.
const RETURN_AT: f32 = 2.0;
const SNIFF_SECS: f32 = 1.0;
const CLOSE: f32 = 25.0;
/// Не дошёл до следа или точки круга за это время — идёт дальше.
const STEP_SECS: f32 = 3.0;
const TRACE_RANGE: [f32; 2] = [40.0, 160.0];
const SHADOW: f32 = 60.0;
const LOOP_CHANCE: f32 = 0.4;
const LOOP_RADIUS: f32 = 60.0;
const LOOP_POINTS: u32 = 8;
const SHY_SECS: f32 = 1.2;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Notice,
    Flee,
    Return { left: f32 },
    Sniff { t: f32 },
    Circle { point: u32, left: f32 },
    Shy { t: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct SelfScare {
    t: f32,
    phase: Phase,
    trace: Vec2,
    away: Vec2,
    circles: bool,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_self_scare(&mut self) -> Option<SelfScare> {
        let trace = self.scare_trace();
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
        Some(SelfScare {
            t: 0.0,
            phase: Phase::Notice,
            trace,
            away: (self.body.pos - trace).normalized(),
            circles: self.rng.chance(LOOP_CHANCE),
        })
    }

    pub(in crate::sim::stunt) fn step_self_scare(
        &mut self,
        mut act: SelfScare,
        dt: f32,
    ) -> Progress<SelfScare> {
        act.t += dt;
        if act.t >= SECS {
            self.halt();
            return Progress::Dropped;
        }
        match act.phase {
            Phase::Notice => {
                self.scare_look_back(1.0);
                if act.t >= NOTICE_SECS {
                    let fling = act.away * (FLEE_SPEED * self.ui);
                    self.drive = Some(Drive::new(fling).friction(FLEE_FRICTION));
                    act.phase = Phase::Flee;
                }
            }
            Phase::Flee => {
                let since = act.t - NOTICE_SECS;
                self.antic.lift = HOP * (PI * since / HOP_SECS).sin().max(0.0);
                self.scare_look_back((1.0 - since).max(0.0));
                if act.t >= RETURN_AT {
                    self.drive = None;
                    self.brain.push(Directive::MoveTo(act.trace));
                    act.phase = Phase::Return { left: STEP_SECS };
                }
            }
            Phase::Return { left } => {
                if self.scare_reached(act.trace) || left <= dt {
                    self.scare_stand();
                    act.phase = Phase::Sniff { t: 0.0 };
                } else {
                    act.phase = Phase::Return { left: left - dt };
                }
            }
            Phase::Sniff { t } => {
                let t = t + dt;
                self.scare_sniff(t);
                if t < SNIFF_SECS {
                    act.phase = Phase::Sniff { t };
                } else if act.circles {
                    self.drive = None;
                    let first = self.scare_clamped(loop_point(&act, 0, self.ui));
                    self.brain.push(Directive::MoveTo(first));
                    act.phase = Phase::Circle {
                        point: 0,
                        left: STEP_SECS,
                    };
                } else {
                    self.halt();
                    return Progress::Finished;
                }
            }
            Phase::Circle { point, left } => {
                let goal = self.scare_clamped(loop_point(&act, point, self.ui));
                if !(self.scare_reached(goal) || left <= dt) {
                    act.phase = Phase::Circle {
                        point,
                        left: left - dt,
                    };
                } else if point + 1 == LOOP_POINTS {
                    self.scare_stand();
                    act.phase = Phase::Shy { t: 0.0 };
                } else {
                    let next = self.scare_clamped(loop_point(&act, point + 1, self.ui));
                    self.brain.push(Directive::MoveTo(next));
                    act.phase = Phase::Circle {
                        point: point + 1,
                        left: STEP_SECS,
                    };
                }
            }
            Phase::Shy { t } => {
                let t = t + dt;
                if t >= SHY_SECS {
                    self.halt();
                    return Progress::Finished;
                }
                self.antic.head_pitch = -0.8;
                self.antic.neck_up = -5.0;
                self.antic.lids = 0.4;
                act.phase = Phase::Shy { t };
            }
        }
        Progress::Going(act)
    }

    /// Свой след позади: ближайший отпечаток за спиной, иначе «тень» рядом.
    fn scare_trace(&self) -> Vec2 {
        let (near, far) = (TRACE_RANGE[0] * self.ui, TRACE_RANGE[1] * self.ui);
        let behind = |pos: &Vec2| {
            let from = *pos - self.body.pos;
            (near..=far).contains(&from.length()) && from.dot(self.body.heading) < 0.0
        };
        let trace = self
            .footprints
            .iter()
            .map(|print| print.pos)
            .filter(behind)
            .min_by(|a, b| {
                let (da, db) = (a.distance(self.body.pos), b.distance(self.body.pos));
                da.total_cmp(&db)
            })
            .unwrap_or(self.body.pos - self.body.heading * (SHADOW * self.ui));
        self.scare_clamped(trace)
    }

    fn scare_clamped(&self, point: Vec2) -> Vec2 {
        self.bounds.clamp(point, self.edge_margin())
    }

    fn scare_reached(&self, spot: Vec2) -> bool {
        self.body.pos.distance(spot) <= CLOSE * self.ui
    }

    /// Стоит на месте, мозг не уводит дальше.
    fn scare_stand(&mut self) {
        self.halt();
        self.drive = Some(Drive::new(Vec2::ZERO));
    }

    /// Смотрит назад на след, глаза круглые.
    fn scare_look_back(&mut self, amount: f32) {
        self.antic.eyes = Eyes::Wide;
        self.antic.head_yaw = 1.8 * amount;
        self.antic.neck_up = 2.0 * amount;
    }

    /// Голова вниз, кивает.
    fn scare_sniff(&mut self, t: f32) {
        let nod = (TAU * 3.0 * t).sin();
        self.antic.neck_up = -4.0;
        self.antic.neck_forward = 6.0;
        self.antic.head_pitch = -0.5 + 0.15 * nod;
    }
}

/// Точка круга вокруг следа.
fn loop_point(act: &SelfScare, index: u32, ui: f32) -> Vec2 {
    let angle = TAU * index as f32 / LOOP_POINTS as f32;
    act.trace + Vec2::from_angle(angle) * (LOOP_RADIUS * ui)
}
