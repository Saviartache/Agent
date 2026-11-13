//! Полоскание: несёт яблоко к луже воды, полощет его передними лапами в брызгах,
//! съедает в три укуса; лужа высыхает.

use std::f32::consts::TAU;

use super::super::Progress;
use super::PAWS_REACH;
use crate::behavior::Directive;
use crate::effects::mud::Liquid;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с, частоты — Гц.
const PUDDLE_RADIUS: f32 = 26.0;
const PUDDLE_TTL: f32 = 20.0;
const PUDDLE_DISTANCE: [f32; 2] = [140.0, 260.0];
const PUDDLE_MIN: f32 = 100.0;
const APPLE_TTL: f32 = 30.0;
/// Допуск места: не теснее, чем приходит ходьба.
const REACH_SLACK: f32 = 9.0;
const WALK_SECS: f32 = 10.0;
const RINSE_SECS: f32 = 3.0;
const RINSE_HZ: f32 = 2.5;
/// Размах полоскания поперёк взгляда.
const RINSE_SWING: f32 = 5.0;
const SPLASH_GAP: f32 = 0.35;
const BITES: u8 = 3;
const BITE_SECS: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk(f32),
    Rinse { t: f32, splash: f32 },
    Eat(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Rinse {
    puddle: Vec2,
    apple: PropId,
    face: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_rinse(&mut self) -> Option<Rinse> {
        let puddle = self.spot_away(PUDDLE_DISTANCE, PUDDLE_MIN, PUDDLE_RADIUS)?;
        self.mud.pour_liquid(
            puddle,
            PUDDLE_RADIUS * self.ui,
            PUDDLE_TTL,
            Liquid::Water,
            &mut self.rng,
        );
        let apple = self
            .props
            .spawn(PropKind::Apple, self.body.pos, 0.0, self.ui, APPLE_TTL);
        let (face, spot) = self.reach_for(puddle, PAWS_REACH);
        self.brain.push(Directive::MoveTo(spot));
        self.hold_in_muzzle(apple);
        Some(Rinse {
            puddle,
            apple,
            face,
            phase: Phase::Walk(WALK_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_rinse(&mut self, mut act: Rinse, dt: f32) -> Progress<Rinse> {
        if self.props.get(act.apple).is_none() {
            return Progress::Dropped;
        }
        act.phase = match act.phase {
            Phase::Walk(left) => {
                self.hold_in_muzzle(act.apple);
                let (face, spot) = self.reach_for(act.puddle, PAWS_REACH);
                if self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                    self.halt();
                    act.face = face;
                    Phase::Rinse {
                        t: 0.0,
                        splash: 0.0,
                    }
                } else if left > dt {
                    Phase::Walk(left - dt)
                } else {
                    self.props.remove(act.apple);
                    return Progress::Dropped;
                }
            }
            Phase::Rinse { t, splash } => self.rinse(&act, t + dt, splash - dt),
            Phase::Eat(spent) => match self.eat_rinsed(act.apple, spent, dt) {
                Some(phase) => phase,
                None => {
                    self.mud.dry_at(act.puddle);
                    return Progress::Finished;
                }
            },
        };
        Progress::Going(act)
    }

    fn rinse(&mut self, act: &Rinse, t: f32, splash: f32) -> Phase {
        self.body.heading = act.face;
        self.paws_down(1.0);
        let swing = (TAU * RINSE_HZ * t).sin();
        self.antic.wobble = 0.8 * swing;
        self.antic.head_yaw = 0.15 * swing;
        let across = Vec2::new(-act.face.y, act.face.x) * (RINSE_SWING * swing * self.ui);
        let at = self.muzzle_ground() + across;
        if let Some(apple) = self.props.get_mut(act.apple) {
            (apple.pos, apple.z, apple.vel, apple.vz) = (at, 0.0, Vec2::ZERO, 0.0);
        }
        let splash = if splash <= 0.0 {
            self.burst(Burst::Water, at);
            SPLASH_GAP
        } else {
            splash
        };
        if t >= RINSE_SECS {
            return Phase::Eat(0.0);
        }
        Phase::Rinse { t, splash }
    }

    /// Ест чистое яблоко с поднятой мордой; `None` — съедено.
    fn eat_rinsed(&mut self, id: PropId, spent: f32, dt: f32) -> Option<Phase> {
        self.paws_down(0.3);
        let bite = (spent / BITE_SECS) as u8;
        let next = spent + dt;
        self.antic.mouth = (TAU * next / BITE_SECS).sin().max(0.0);
        self.hold_in_muzzle(id);
        let apple = self.props.get_mut(id)?;
        if (next / BITE_SECS) as u8 > bite {
            apple.stage += 1;
            if apple.stage >= BITES {
                self.props.remove(id);
                return None;
            }
        }
        Some(Phase::Eat(next))
    }
}
