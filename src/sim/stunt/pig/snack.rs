//! Перекус: ест яблоко в четыре укуса; переела — раздулась и катится.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::rig::GROUND_SQUASH;
use crate::sim::{Sim, drive::Drive};

/// Расстояния — логические px (design px — у тела), время — с, частоты — Гц.
const APPLE_DROP: f32 = 40.0;
const APPLE_TTL: f32 = 40.0;
/// Пятачок у земли от точки под телом, design px (`snout_down(1.0)`).
pub(in crate::sim::stunt::pig) const SNOUT_REACH: f32 = 22.0;
/// Допуск места, логические px: не теснее, чем приходит ходьба.
const REACH_SLACK: f32 = 9.0;
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
        face: Vec2,
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
        // Падает прямо под пятачок
        let heading = self.body.heading.normalized();
        let ahead = Vec2::new(heading.x, heading.y * GROUND_SQUASH) * (SNOUT_REACH * self.sprite);
        let at = self
            .bounds
            .nearest(self.body.pos)
            .clamp(self.body.pos + ahead, self.edge_margin());
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
                let (face, spot) = self.reach_for(found.pos, SNOUT_REACH);
                if found.resting() && self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                    self.halt();
                    Phase::Bite {
                        apple,
                        face,
                        spent: 0.0,
                    }
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
            Phase::Bite { apple, face, spent } => match self.bite(apple, face, spent, dt) {
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

    /// Идёт к яблоку и встаёт так, чтобы дотянуться пятачком.
    fn approach(&mut self, apple: Vec2) {
        let (_, spot) = self.reach_for(apple, SNOUT_REACH);
        self.brain.push(Directive::MoveTo(spot));
    }

    /// `None` — съедено, либо яблоко пропало.
    fn bite(&mut self, id: PropId, face: Vec2, spent: f32, dt: f32) -> Option<Phase> {
        let swing = (PI * spent / BITE_SECS).sin().max(0.0);
        self.body.heading = face;
        self.snout_down(0.7 + 0.3 * swing);
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
            face,
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

    /// Рыло вперёд и вниз, `depth` 0…1; при 1 пятачок касается земли в `SNOUT_REACH`
    /// перед телом, а голова выходит вперёд передних ног, а не проваливается между ними.
    pub(in crate::sim::stunt::pig) fn snout_down(&mut self, depth: f32) {
        self.antic.neck_forward = 25.0 * depth;
        self.antic.neck_up = -20.0 * depth;
        self.antic.head_pitch = -1.4 * depth;
    }
}
