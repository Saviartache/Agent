//! Подарок: уходит за ближний боковой край монитора, возвращается с предметом своего вида
//! в клюве или пасти, кладёт его у курсора (без курсора — посреди монитора) и гордо ждёт.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::{Rect, Vec2};
use crate::rig::RigKind;
use crate::sim::{Drive, Sim};

/// Время — с, расстояния — логические px, если не сказано «design».
const TIMEOUT: f32 = 20.0;
/// Насколько тело может зайти за край, design px: уходит целиком.
const OVERHANG: f32 = 110.0;
const WAIT_SECS: f32 = 1.5;
/// Добежал до края, когда осталось столько, px.
const EDGE_REACH: f32 = 14.0;
/// Кладёт предмет на таком расстоянии от цели.
const PLACE_DISTANCE: f32 = 40.0;
const PROUD_SECS: f32 = 1.6;
/// Предмет лежит столько после того, как его положили.
const GIFT_LIFE: f32 = 30.0;
/// Запас жизни, пока предмет несут.
const CARRY_LIFE: f32 = 40.0;
/// Высота предмета в клюве у спрайта без головы, design px.
const HEAD_DROP: f32 = 4.0;

#[derive(Debug)]
enum Phase {
    /// Бежит к краю.
    Run,
    /// Уходит за край.
    Leave,
    /// Ждёт за краем.
    Wait(f32),
    /// Возвращается с предметом.
    Return,
    /// Несёт к цели.
    Bring,
    Proud(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Gift {
    /// −1 — левый край, 1 — правый.
    side: f32,
    phase: Phase,
    item: Option<PropId>,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_gift(&mut self) -> Option<Gift> {
        let area = self.bounds.nearest(self.body.pos);
        let side = if self.body.pos.x - area.min.x < area.max.x - self.body.pos.x {
            -1.0
        } else {
            1.0
        };
        let at = Vec2::new(self.side_edge(area, side), self.body.pos.y);
        self.brain.push(Directive::RunTo(at));
        Some(Gift {
            side,
            phase: Phase::Run,
            item: None,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_gift(&mut self, mut act: Gift, dt: f32) -> Progress<Gift> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let area = self.bounds.nearest(self.body.pos);
        let speed = self.character.motion().walk_speed * self.ui;
        let out = Vec2::new(act.side * speed, 0.0);
        match act.phase {
            Phase::Run => {
                let edge = self.side_edge(area, act.side);
                if (self.body.pos.x - edge).abs() <= EDGE_REACH * self.ui {
                    self.halt();
                    self.drive = Some(self.hidden_drive(out));
                    act.phase = Phase::Leave;
                }
            }
            Phase::Leave => {
                self.drive = Some(self.hidden_drive(out));
                if self.bumped.is_some() {
                    act.phase = Phase::Wait(WAIT_SECS);
                }
            }
            Phase::Wait(left) => {
                self.drive = Some(self.hidden_drive(Vec2::ZERO));
                act.phase = if left > dt {
                    Phase::Wait(left - dt)
                } else {
                    act.item = Some(self.spawn_gift(area));
                    Phase::Return
                };
            }
            Phase::Return => {
                self.drive = Some(self.hidden_drive(-out));
                let edge = self.side_edge(area, act.side);
                if (self.body.pos.x - edge) * act.side <= 0.0 {
                    self.drive = None;
                    act.phase = Phase::Bring;
                }
            }
            Phase::Bring => {
                let goal = self.cursor.unwrap_or(area.center());
                self.brain.push(Directive::MoveTo(goal));
                if self.body.pos.distance(goal) <= PLACE_DISTANCE * self.ui {
                    self.halt();
                    if let Some(item) = act.item.and_then(|id| self.props.get_mut(id)) {
                        item.expire_in(GIFT_LIFE);
                    }
                    act.item = None;
                    self.burst(Burst::Hearts, self.head());
                    self.perform_quietly(self.character.actions().celebrate);
                    act.phase = Phase::Proud(PROUD_SECS);
                }
            }
            Phase::Proud(left) if left > dt => act.phase = Phase::Proud(left - dt),
            Phase::Proud(_) => return Progress::Finished,
        }
        self.carry_gift(&act);
        Progress::Going(act)
    }

    /// Ближний к `side` край монитора, по которому ходит.
    fn side_edge(&self, area: Rect, side: f32) -> f32 {
        if side < 0.0 {
            area.min.x + self.edge_margin()
        } else {
            area.max.x - self.edge_margin()
        }
    }

    fn hidden_drive(&self, vel: Vec2) -> Drive {
        Drive::new(vel).overhang(OVERHANG * self.sprite)
    }

    fn spawn_gift(&mut self, area: Rect) -> PropId {
        let kind = match self.character.rig() {
            Some(RigKind::Goose) if self.rng.chance(0.5) => PropKind::Feather,
            Some(RigKind::Pig) => PropKind::Acorn,
            _ => PropKind::Pebble,
        };
        let at = area.clamp(self.body.pos, 0.0);
        self.props.spawn(kind, at, 0.0, self.ui, CARRY_LIFE)
    }

    /// Предмет у головы: точка на земле под клювом и высота до него.
    fn carry_gift(&mut self, act: &Gift) {
        let (Some(id), Phase::Return | Phase::Bring) = (act.item, &act.phase) else {
            return;
        };
        let (head, ground) = (self.head(), self.body.pos);
        let z = (ground.y - head.y - HEAD_DROP * self.sprite).max(0.0);
        if let Some(item) = self.props.get_mut(id) {
            item.pos = Vec2::new(head.x, ground.y);
            item.z = z;
            item.vel = Vec2::ZERO;
            item.vz = 0.0;
        }
    }
}
