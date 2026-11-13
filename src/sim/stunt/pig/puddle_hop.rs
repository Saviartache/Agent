//! Шлёпанье: прыгает по цепочке луж, след цепочкой.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, design px, с.
const PUDDLE_RADIUS: f32 = 16.0;
const PUDDLE_TTL: f32 = 20.0;
const FIRST_AT: f32 = 100.0;
const STRIDE: f32 = 70.0;
const ZIG: f32 = 25.0;
const MIN_PUDDLES: usize = 3;
const MAX_PUDDLES: usize = 4;
const WALK_SECS: f32 = 10.0;
const ARRIVE: f32 = 12.0;
const HOP_HEIGHT: f32 = 16.0;
const HOP_SECS: f32 = 0.45;
const LAND_PAUSE: f32 = 0.15;
const DIRTY: f32 = 0.25;
const CHEER_SECS: f32 = 1.2;
const TIMEOUT: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk,
    Hop,
    Land,
    Cheer,
}

#[derive(Clone, Debug)]
pub(in crate::sim) struct PuddleHop {
    puddles: Vec<Vec2>,
    /// Номер лужи, куда идёт или прыгает.
    to: usize,
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_puddle_hop(&mut self) -> Option<PuddleHop> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin() + PUDDLE_RADIUS * self.ui;
        let count = MIN_PUDDLES + self.pick(MAX_PUDDLES - MIN_PUDDLES + 1);
        let way = Vec2::from_angle(self.rng.range(0.0, TAU));
        let side = Vec2::new(-way.y, way.x);
        let puddles: Vec<Vec2> = (0..count)
            .map(|i| {
                let zig = if i % 2 == 0 { ZIG } else { -ZIG };
                let along = FIRST_AT + STRIDE * i as f32;
                area.clamp(self.body.pos + (way * along + side * zig) * self.ui, margin)
            })
            .collect();
        // Прижатые к краю лужи слиплись бы в одну
        let gaps_ok = puddles
            .windows(2)
            .all(|pair| pair[0].distance(pair[1]) >= STRIDE * 0.6 * self.ui);
        if !gaps_ok {
            return None;
        }
        for puddle in &puddles {
            self.mud
                .pour(*puddle, PUDDLE_RADIUS * self.ui, PUDDLE_TTL, &mut self.rng);
        }
        self.brain.push(Directive::MoveTo(puddles[0]));
        Some(PuddleHop {
            puddles,
            to: 0,
            phase: Phase::Walk,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_puddle_hop(
        &mut self,
        mut act: PuddleHop,
        dt: f32,
    ) -> Progress<PuddleHop> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            return Progress::Dropped;
        }
        let mut next = None;
        match act.phase {
            Phase::Walk => {
                if self.body.pos.distance(act.puddles[0]) <= ARRIVE * self.ui {
                    self.halt();
                    act.to = 1;
                    next = Some(self.start_hop(&act));
                } else if act.t > WALK_SECS {
                    return Progress::Dropped;
                }
            }
            Phase::Hop => {
                let target = act.puddles[act.to];
                self.antic.lift = HOP_HEIGHT * (PI * act.t / HOP_SECS).sin().max(0.0);
                if act.t >= HOP_SECS {
                    self.drive = None;
                    self.body.pos = target;
                    self.halt();
                    self.burst(Burst::Mud, target);
                    self.cond.muddy = (self.cond.muddy + DIRTY).min(1.0);
                    next = Some(Phase::Land);
                }
            }
            Phase::Land => {
                if act.t >= LAND_PAUSE {
                    if act.to + 1 < act.puddles.len() {
                        act.to += 1;
                        next = Some(self.start_hop(&act));
                    } else {
                        self.perform_quietly(self.character.actions().celebrate);
                        next = Some(Phase::Cheer);
                    }
                }
            }
            Phase::Cheer => {
                if act.t >= CHEER_SECS {
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

    /// Толчок к луже `act.to` ровно на время прыжка.
    fn start_hop(&mut self, act: &PuddleHop) -> Phase {
        let vel = (act.puddles[act.to] - self.body.pos) * (1.0 / HOP_SECS);
        self.drive = Some(Drive::new(vel));
        Phase::Hop
    }
}
