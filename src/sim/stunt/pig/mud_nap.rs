//! Сон в луже: пузыри, встаёт грязнее обычного.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Логические px, с.
const PUDDLE_RADIUS: f32 = 40.0;
const PUDDLE_TTL: f32 = 18.0;
const PUDDLE_DISTANCE: [f32; 2] = [150.0, 300.0];
const PUDDLE_MIN_DISTANCE: f32 = 80.0;
/// Ближе этой доли радиуса к середине — уже в луже.
const ENTER: f32 = 0.6;
const NAP_SECS: f32 = 6.0;
const BUBBLE_GAP: f32 = 0.6;
/// Пузыри — в пределах этой доли радиуса.
const BUBBLE_REACH: f32 = 0.8;
const SETTLE_SECS: f32 = 0.5;
const ROLL: f32 = 0.5;
const TIMEOUT: f32 = 16.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run,
    Nap { bubble: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct MudNap {
    puddle: Vec2,
    phase: Phase,
    t: f32,
    total: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_mud_nap(&mut self) -> Option<MudNap> {
        let puddle = self.spot_away(PUDDLE_DISTANCE, PUDDLE_MIN_DISTANCE, PUDDLE_RADIUS)?;
        self.mud
            .pour(puddle, PUDDLE_RADIUS * self.ui, PUDDLE_TTL, &mut self.rng);
        self.brain.push(Directive::RunTo(puddle));
        Some(MudNap {
            puddle,
            phase: Phase::Run,
            t: 0.0,
            total: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_mud_nap(
        &mut self,
        mut act: MudNap,
        dt: f32,
    ) -> Progress<MudNap> {
        act.t += dt;
        act.total += dt;
        if act.total > TIMEOUT {
            return Progress::Dropped;
        }
        match act.phase {
            Phase::Run => {
                if self.body.pos.distance(act.puddle) <= ENTER * PUDDLE_RADIUS * self.ui {
                    self.halt();
                    act.phase = Phase::Nap { bubble: 0.0 };
                    act.t = 0.0;
                }
            }
            Phase::Nap { bubble } => {
                let settle = (act.t / SETTLE_SECS).min(1.0);
                self.antic.crouch = 6.0 * settle;
                self.antic.tuck = 1.0;
                self.antic.lids = settle;
                self.antic.roll = ROLL * settle;
                self.cond.muddy = self.cond.muddy.max((act.t / NAP_SECS).min(1.0));
                if act.t >= NAP_SECS {
                    self.cond.muddy = 1.0;
                    self.cond.snout = 1.0;
                    self.mud.dry_at(act.puddle);
                    return Progress::Finished;
                }
                let bubble = if bubble <= 0.0 {
                    let reach = PUDDLE_RADIUS * BUBBLE_REACH * self.ui;
                    let spot = act.puddle
                        + Vec2::new(self.rng.range(-reach, reach), self.rng.range(-reach, reach));
                    self.burst(Burst::Bubbles, spot);
                    BUBBLE_GAP
                } else {
                    bubble - dt
                };
                act.phase = Phase::Nap { bubble };
            }
        }
        Progress::Going(act)
    }
}
