//! Спотыкается о карточку: идёт к её нижнему краю, падает носом, встаёт и отряхивается.
//! Карточку не двигает и не закрывает — только пыль у её края.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::animation::tween::Easing;
use crate::behavior::Directive;
use crate::effects::items::{ItemId, ItemPhase};
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::{Drive, Sim};

/// Расстояния — логические px, время — с.
const TIMEOUT: f32 = 12.0;
/// Идёт к точке под нижним краем карточки.
const BELOW: f32 = 16.0;
const TRIP_REACH: f32 = 46.0;
const FALL_SECS: f32 = 0.4;
const LIE_SECS: f32 = 0.7;
const RISE_SECS: f32 = 0.6;
const SHAKE_SECS: f32 = 0.9;
const LUNGE: f32 = 140.0;
const LUNGE_FRICTION: f32 = 6.0;
/// Нос вниз, рад.
const NOSE_DOWN: f32 = 1.0;

#[derive(Debug)]
enum Phase {
    Walk,
    Fall(f32),
    Lie(f32),
    Rise(f32),
    Shake(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct TripOverCard {
    card: ItemId,
    phase: Phase,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_trip_over_card(&mut self) -> Option<TripOverCard> {
        let card = self
            .items
            .iter()
            .filter(|item| item.phase() == ItemPhase::Placed)
            .min_by(|a, b| {
                let [da, db] = [a, b].map(|item| item.pos().distance(self.body.pos));
                da.total_cmp(&db)
            })?
            .id();
        Some(TripOverCard {
            card,
            phase: Phase::Walk,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_trip_over_card(
        &mut self,
        mut act: TripOverCard,
        dt: f32,
    ) -> Progress<TripOverCard> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let Some(card) = self.items.iter().find(|item| item.id() == act.card) else {
            return Progress::Dropped;
        };
        let edge = card.pos();
        let below = self
            .bounds
            .clamp(edge + Vec2::new(0.0, BELOW * self.ui), 0.0);
        match act.phase {
            Phase::Walk => {
                self.brain.push(Directive::MoveTo(below));
                if self.body.pos.distance(below) <= TRIP_REACH * self.ui {
                    let ahead = self.body.heading.normalized() * (LUNGE * self.ui);
                    self.halt();
                    self.drive = Some(Drive::new(ahead).friction(LUNGE_FRICTION));
                    self.burst(Burst::Dust, edge);
                    act.phase = Phase::Fall(0.0);
                }
            }
            Phase::Fall(t) => {
                self.sprawl(Easing::InQuad.apply(t / FALL_SECS));
                act.phase = if t + dt < FALL_SECS {
                    Phase::Fall(t + dt)
                } else {
                    self.burst(Burst::Dust, self.head());
                    Phase::Lie(LIE_SECS)
                };
            }
            Phase::Lie(left) => {
                self.sprawl(1.0);
                act.phase = if left > dt {
                    Phase::Lie(left - dt)
                } else {
                    // Встаёт на месте
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    Phase::Rise(0.0)
                };
            }
            Phase::Rise(t) => {
                self.sprawl(1.0 - Easing::InOutCubic.apply(t / RISE_SECS));
                act.phase = if t + dt < RISE_SECS {
                    Phase::Rise(t + dt)
                } else {
                    self.burst(Burst::Dust, self.body.pos);
                    Phase::Shake(SHAKE_SECS)
                };
            }
            Phase::Shake(left) => {
                self.antic.wobble = 3.0 * (TAU * 4.0 * left).sin();
                if left <= dt {
                    return Progress::Finished;
                }
                act.phase = Phase::Shake(left - dt);
            }
        }
        Progress::Going(act)
    }

    /// Лежит носом в землю: `u` — 0 стоит, 1 упал.
    fn sprawl(&mut self, u: f32) {
        self.antic.pitch = -NOSE_DOWN * u;
        self.antic.neck_forward = u;
        self.antic.neck_up = -u;
        self.antic.head_pitch = -0.6 * u;
        self.antic.crouch = 0.5 * u;
    }
}
