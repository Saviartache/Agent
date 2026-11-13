//! Перекус: ест яблоко в четыре укуса; переела — раздулась и катится.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::{Sim, drive::Drive};

/// Расстояния — логические px (design px — у тела), время — с, частоты — Гц.
const APPLE_DISTANCE: f32 = 120.0;
const APPLE_DROP: f32 = 40.0;
const APPLE_TTL: f32 = 40.0;
/// Свинка встаёт на таком расстоянии от яблока, design px.
const REACH: f32 = 36.0;
const REACH_SLACK: f32 = 10.0;
const WALK_SECS: f32 = 8.0;
const REAPPROACH_GAP: f32 = 0.4;
const BITES: u8 = 4;
const BITE_SECS: f32 = 0.6;
/// Укус приходится на эту секунду тычка.
const BITE_AT: f32 = 0.3;
const OVEREAT_CHANCE: f32 = 0.35;
const ROLL_SPEED: f32 = 80.0;
const ROLL_FRICTION: f32 = 1.0;
const ROLL_AMPLITUDE: f32 = 0.6;
const ROLL_HZ: f32 = 1.5;
const ROLL_SECS: f32 = 2.0;
const DEFLATED: f32 = 0.2;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk {
        apple: PropId,
        left: f32,
        retarget: f32,
    },
    Bite {
        apple: PropId,
        spent: f32,
    },
    Roll(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Snack {
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_snack(&mut self) -> Option<Snack> {
        let ahead = self.body.pos + self.body.heading.normalized() * (APPLE_DISTANCE * self.ui);
        let at = self
            .bounds
            .nearest(self.body.pos)
            .clamp(ahead, self.edge_margin());
        let apple = self.props.spawn(
            PropKind::Apple,
            at,
            APPLE_DROP * self.ui,
            self.ui,
            APPLE_TTL,
        );
        self.approach(at);
        Some(Snack {
            phase: Phase::Walk {
                apple,
                left: WALK_SECS,
                retarget: REAPPROACH_GAP,
            },
        })
    }

    pub(in crate::sim::stunt) fn step_snack(&mut self, act: Snack, dt: f32) -> Progress<Snack> {
        let phase = match act.phase {
            Phase::Walk {
                apple,
                left,
                retarget,
            } => {
                let Some(found) = self.props.get(apple).copied() else {
                    return Progress::Dropped;
                };
                let near = (REACH + REACH_SLACK) * self.sprite;
                if found.resting() && self.body.pos.distance(found.pos) <= near {
                    self.halt();
                    Phase::Bite { apple, spent: 0.0 }
                } else if left <= dt {
                    self.props.remove(apple);
                    return Progress::Dropped;
                } else if retarget <= dt {
                    self.approach(found.pos);
                    Phase::Walk {
                        apple,
                        left: left - dt,
                        retarget: REAPPROACH_GAP,
                    }
                } else {
                    Phase::Walk {
                        apple,
                        left: left - dt,
                        retarget: retarget - dt,
                    }
                }
            }
            Phase::Bite { apple, spent } => match self.bite(apple, spent, dt) {
                Some(phase) => phase,
                None => return self.overeat_or_finish(),
            },
            Phase::Roll(left) => {
                self.cond.bloated = 1.0;
                self.antic.roll = ROLL_AMPLITUDE * (TAU * ROLL_HZ * (ROLL_SECS - left)).sin();
                if left <= dt {
                    self.drive = None;
                    self.halt();
                    self.burst(Burst::Puff, self.body.pos);
                    self.cond.bloated = DEFLATED;
                    return Progress::Finished;
                }
                Phase::Roll(left - dt)
            }
        };
        Progress::Going(Snack { phase })
    }

    /// Идёт к яблоку и встаёт, не доходя до него.
    fn approach(&mut self, apple: Vec2) {
        let gap = (apple - self.body.pos).normalized() * (REACH * self.sprite);
        self.brain.push(Directive::MoveTo(apple - gap));
    }

    /// `None` — съедено, либо яблоко пропало.
    fn bite(&mut self, id: PropId, spent: f32, dt: f32) -> Option<Phase> {
        let swing = (PI * spent / BITE_SECS).sin().max(0.0);
        self.antic.neck_forward = 5.0 * swing;
        self.antic.head_pitch = -0.5 * swing;
        self.antic.mouth = swing;
        let apple = self.props.get_mut(id)?;
        if spent < BITE_AT && spent + dt >= BITE_AT {
            apple.stage += 1;
            let (at, eaten) = (apple.pos, apple.stage >= BITES);
            self.burst(Burst::Dust, at);
            if eaten {
                self.props.remove(id);
                return None;
            }
        }
        // Следующий укус без паузы
        let next = spent + dt;
        Some(Phase::Bite {
            apple: id,
            spent: if next >= BITE_SECS { 0.0 } else { next },
        })
    }

    fn overeat_or_finish(&mut self) -> Progress<Snack> {
        if !self.rng.chance(OVEREAT_CHANCE) {
            return Progress::Finished;
        }
        let ahead = self.body.heading.normalized() * (ROLL_SPEED * self.ui);
        self.drive = Some(Drive::new(ahead).friction(ROLL_FRICTION));
        Progress::Going(Snack {
            phase: Phase::Roll(ROLL_SECS),
        })
    }
}
