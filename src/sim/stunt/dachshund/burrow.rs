//! Нора: такса бежит к ближнему боковому краю, роет передними лапами — комья летят,
//! остаются ямки, — ныряет в нору за край, виляя оттуда хвостом, и выбирается
//! обратно с грязным носом.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, design px для заступа за край, с, Гц.
const APPROACH: f32 = 50.0;
const ARRIVE: f32 = 24.0;
const WALK_SECS: f32 = 10.0;
const DIG_SECS: f32 = 2.4;
const DIG_HZ: f32 = 6.0;
const DIRT_GAP: f32 = 0.15;
const PIT_GAP: f32 = 0.8;
const PIT_RADIUS: f32 = 7.0;
const PIT_TTL: f32 = 10.0;
const CRAWL_SPEED: f32 = 90.0;
/// Насколько тело заходит за край: снаружи остаются зад и хвост.
const OVERHANG: f32 = 30.0;
const HIDDEN_SECS: f32 = 1.5;
const POP_SPEED: f32 = 260.0;
const POP_FRICTION: f32 = 5.0;
const STOPPED: f32 = 20.0;
const SHAKE_SECS: f32 = 0.8;
const SHAKE_HZ: f32 = 9.0;
const SNOUT: f32 = 0.8;
const TIMEOUT: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Walk,
    Dig,
    Dive,
    Hidden,
    Out,
    Shake,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Burrow {
    /// Куда роет: `(-1, 0)` — у левого края, `(1, 0)` — у правого.
    toward: Vec2,
    phase: Phase,
    t: f32,
    total: f32,
    dirt_in: f32,
    pit_in: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_burrow(&mut self) -> Option<Burrow> {
        let area = self.bounds.nearest(self.body.pos);
        let left = self.body.pos.x - area.min.x < area.max.x - self.body.pos.x;
        let toward = Vec2::new(if left { -1.0 } else { 1.0 }, 0.0);
        let edge = if left { area.min.x } else { area.max.x };
        let gate = Vec2::new(edge, self.body.pos.y) - toward * (APPROACH * self.ui);
        self.brain
            .push(Directive::MoveTo(area.clamp(gate, self.edge_margin())));
        Some(Burrow {
            toward,
            phase: Phase::Walk,
            t: 0.0,
            total: 0.0,
            dirt_in: 0.0,
            pit_in: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_burrow(
        &mut self,
        mut act: Burrow,
        dt: f32,
    ) -> Progress<Burrow> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            return Progress::Dropped;
        }
        let overhang = OVERHANG * self.sprite;
        let next = match act.phase {
            Phase::Walk => {
                let area = self.bounds.nearest(self.body.pos);
                let edge = if act.toward.x < 0.0 {
                    area.min.x
                } else {
                    area.max.x
                };
                let near = (edge - self.body.pos.x).abs()
                    <= (APPROACH + ARRIVE) * self.ui + self.edge_margin();
                if near {
                    self.halt();
                    Some(Phase::Dig)
                } else if act.t > WALK_SECS {
                    return Progress::Dropped;
                } else {
                    None
                }
            }
            Phase::Dig => {
                self.burrow_dig(&mut act, dt);
                (act.t >= DIG_SECS).then(|| {
                    let vel = act.toward * (CRAWL_SPEED * self.ui);
                    self.drive = Some(Drive::new(vel).overhang(overhang));
                    Phase::Dive
                })
            }
            Phase::Dive => {
                self.antic.pitch = -0.2;
                self.antic.crouch = 2.0;
                self.bumped.is_some().then(|| {
                    self.drive = Some(Drive::new(Vec2::ZERO).overhang(overhang));
                    Phase::Hidden
                })
            }
            Phase::Hidden => {
                self.antic.wag = 1.0;
                self.antic.crouch = 2.0;
                self.antic.wobble = 0.8 * (TAU * DIG_HZ * act.t).sin();
                (act.t >= HIDDEN_SECS).then(|| {
                    let vel = act.toward * (-POP_SPEED * self.ui);
                    self.drive = Some(Drive::new(vel).friction(POP_FRICTION).overhang(overhang));
                    self.burst(Burst::Dirt, self.body.pos);
                    self.cond.snout = SNOUT;
                    Phase::Out
                })
            }
            // Первый шаг скорость тела ещё старая
            Phase::Out => (act.t > dt && self.body.vel.length() < STOPPED * self.ui).then(|| {
                self.drive = None;
                self.halt();
                self.burst(Burst::Dust, self.body.pos);
                Phase::Shake
            }),
            Phase::Shake => {
                self.antic.wobble = 2.0 * (TAU * SHAKE_HZ * act.t).sin();
                self.antic.head_roll = 0.4 * (TAU * SHAKE_HZ * act.t).sin();
                if act.t >= SHAKE_SECS {
                    return Progress::Finished;
                }
                None
            }
        };
        if let Some(phase) = next {
            act.phase = phase;
            act.t = 0.0;
        }
        Progress::Going(act)
    }

    /// Передом в землю, лапы гребут: комья назад, на земле ямки.
    fn burrow_dig(&mut self, act: &mut Burrow, dt: f32) {
        self.body.heading = act.toward;
        self.antic.pitch = -0.25;
        self.antic.neck_forward = 6.0;
        self.antic.neck_up = -9.0;
        self.antic.head_pitch = -0.8;
        self.antic.wobble = 1.0 * (TAU * DIG_HZ * act.t).sin();
        self.antic.wag = 0.6;
        act.dirt_in -= dt;
        if act.dirt_in <= 0.0 {
            act.dirt_in = DIRT_GAP;
            self.burst(Burst::Dirt, self.muzzle_ground());
        }
        act.pit_in -= dt;
        if act.pit_in <= 0.0 {
            act.pit_in = PIT_GAP;
            self.mud.pour(
                self.muzzle_ground(),
                PIT_RADIUS * self.ui,
                PIT_TTL,
                &mut self.rng,
            );
        }
    }
}
