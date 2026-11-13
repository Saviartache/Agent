//! Юз: гусь бежит к цели, поздно тормозит и проезжает мимо в облаке пыли; потом
//! невозмутимо возвращается пешком.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const DISTANCE: [f32; 2] = [260.0, 340.0];
const MIN_DISTANCE: f32 = 200.0;
/// С запасом на проезд мимо цели.
const EDGE_ROOM: f32 = 150.0;
const BRAKE_AT: f32 = 60.0;
const ARRIVE: f32 = 12.0;
const BOOST: f32 = 1.2;
/// Не медленнее этой доли бега, даже если мозг уже сбросил скорость.
const MIN_SPEED: f32 = 0.5;
const FRICTION: f32 = 2.0;
const STOPPED: f32 = 8.0;
const SKID_LIMIT: f32 = 2.5;
const DUST_GAP: f32 = 0.15;
const LEAN: f32 = 0.35;
const PAUSE_SECS: f32 = 0.5;
const LIMIT: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Run,
    Skid,
    Pause,
    Walk,
}

#[derive(Debug)]
pub(in crate::sim) struct Skid {
    goal: Vec2,
    phase: Phase,
    /// Время в фазе.
    t: f32,
    total: f32,
    dust_in: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_skid(&mut self) -> Option<Skid> {
        let goal = self.spot_away(DISTANCE, MIN_DISTANCE, EDGE_ROOM)?;
        self.brain.push(Directive::RunTo(goal));
        Some(Skid {
            goal,
            phase: Phase::Run,
            t: 0.0,
            total: 0.0,
            dust_in: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_skid(&mut self, mut act: Skid, dt: f32) -> Progress<Skid> {
        act.t += dt;
        act.total += dt;
        if act.total > LIMIT {
            self.drive = None;
            self.halt();
            return Progress::Dropped;
        }
        let to = act.goal - self.body.pos;
        match act.phase {
            Phase::Run => {
                if to.length() <= BRAKE_AT * self.ui {
                    let run = self.character.motion().run_speed * self.ui * MIN_SPEED;
                    let speed = (self.body.vel.length() * BOOST).max(run);
                    let vel = to.normalized() * speed;
                    self.drive = Some(Drive::new(vel).friction(FRICTION));
                    enter(&mut act, Phase::Skid);
                }
            }
            Phase::Skid => {
                self.antic.pitch = LEAN;
                self.antic.crouch = 0.3;
                act.dust_in -= dt;
                if act.dust_in <= 0.0 {
                    act.dust_in = DUST_GAP;
                    self.burst(Burst::Dust, self.body.pos);
                }
                if self.body.vel.length() < STOPPED * self.ui || act.t > SKID_LIMIT {
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    self.halt();
                    enter(&mut act, Phase::Pause);
                }
            }
            Phase::Pause => {
                self.antic.head_yaw = 1.2 * (TAU * act.t / PAUSE_SECS).sin();
                self.antic.lids = 0.3;
                if act.t >= PAUSE_SECS {
                    self.drive = None;
                    self.brain.push(Directive::MoveTo(act.goal));
                    enter(&mut act, Phase::Walk);
                }
            }
            Phase::Walk => {
                self.antic.lids = 0.3;
                if to.length() <= ARRIVE * self.ui {
                    self.halt();
                    return Progress::Finished;
                }
            }
        }
        Progress::Going(act)
    }
}

fn enter(act: &mut Skid, phase: Phase) {
    act.phase = phase;
    act.t = 0.0;
}
