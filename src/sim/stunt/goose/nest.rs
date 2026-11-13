//! Гнездо: гусь сгребает листья в кольцо, садится в середину, встаёт — в гнезде яйцо,
//! которое через пару секунд подмигивает и исчезает.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use super::crumbs::PECK_REACH;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const SPOT_DISTANCE: [f32; 2] = [160.0, 300.0];
const SPOT_MIN_DISTANCE: f32 = 100.0;
const RING_RADIUS: f32 = 35.0;
const HEAPS: usize = 7;
const HEAP_RADIUS: f32 = 9.0;
const HEAP_LEAVES: usize = 10;
/// Листья тают сами.
const LEAVES_TTL: f32 = 20.0;
/// Гусь встал на столько логических px от места, откуда клюв достаёт кучку, — сгребает.
const REACH_SLACK: f32 = 9.0;
/// Дошёл до середины на столько design px — садится.
const CENTER_SLACK: f32 = 7.0;
const RAKE_SECS: f32 = 0.5;
const SIT_SECS: f32 = 2.5;
const EGG_TTL: f32 = 10.0;
const WINK_AFTER: f32 = 2.0;
const WINK_FADE: f32 = 0.4;
/// Не обошёл за это время — сценка сорвалась.
const STEP_SECS: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// Идёт к точке кольца.
    Walk {
        i: usize,
        left: f32,
    },
    /// Сгребает кучку `i`, глядя на неё.
    Rake {
        i: usize,
        face: Vec2,
        t: f32,
    },
    /// Идёт в середину.
    Enter(f32),
    Sit(f32),
    /// Яйцо лежит, ждём подмигивания.
    Egg {
        id: PropId,
        t: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Nest {
    center: Vec2,
    /// С какой точки начали и в какую сторону обходим.
    first: usize,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_nest(&mut self) -> Option<Nest> {
        let center = self.spot_away(SPOT_DISTANCE, SPOT_MIN_DISTANCE, RING_RADIUS + 20.0)?;
        for i in 0..HEAPS {
            let at = self.ring_point(center, i);
            self.leaves.heap(
                at,
                HEAP_RADIUS * self.ui,
                HEAP_LEAVES,
                LEAVES_TTL,
                self.ui,
                &mut self.rng,
            );
        }
        // Обход с ближайшей к гусю точки
        let first = (0..HEAPS)
            .min_by(|a, b| {
                let from = self.body.pos;
                let (a, b) = (self.ring_point(center, *a), self.ring_point(center, *b));
                from.distance(a).total_cmp(&from.distance(b))
            })
            .unwrap_or(0);
        Some(Nest {
            center,
            first,
            phase: Phase::Walk {
                i: 0,
                left: STEP_SECS,
            },
        })
    }

    pub(in crate::sim::stunt) fn step_nest(&mut self, act: Nest, dt: f32) -> Progress<Nest> {
        let phase = match act.phase {
            Phase::Walk { i, left } => {
                let at = self.ring_point(act.center, act.first + i);
                let (face, spot) = self.reach_for(at, PECK_REACH);
                if self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                    self.halt();
                    Phase::Rake { i, face, t: 0.0 }
                } else if left <= dt {
                    return Progress::Dropped;
                } else {
                    // Место сдвигается, пока гусь заходит к кучке
                    self.brain.push(Directive::MoveTo(spot));
                    Phase::Walk { i, left: left - dt }
                }
            }
            Phase::Rake { i, face, t } => {
                let t = t + dt;
                self.body.heading = face;
                self.peck_pose((t / RAKE_SECS * PI).sin());
                if t < RAKE_SECS {
                    Phase::Rake { i, face, t }
                } else if i + 1 < HEAPS {
                    Phase::Walk {
                        i: i + 1,
                        left: STEP_SECS,
                    }
                } else {
                    self.brain.push(Directive::MoveTo(act.center));
                    Phase::Enter(STEP_SECS)
                }
            }
            Phase::Enter(left) => {
                if self.body.pos.distance(act.center) <= CENTER_SLACK * self.sprite {
                    self.halt();
                    Phase::Sit(0.0)
                } else if left <= dt {
                    return Progress::Dropped;
                } else {
                    Phase::Enter(left - dt)
                }
            }
            Phase::Sit(t) => {
                let t = t + dt;
                if t < SIT_SECS {
                    let settle = (t / 0.4).min(1.0);
                    self.antic.crouch = 6.0 * settle;
                    self.antic.tuck = settle;
                    self.antic.lids = 0.5 * settle;
                    Phase::Sit(t)
                } else {
                    let id = self
                        .props
                        .spawn(PropKind::Egg, act.center, 0.0, self.ui, EGG_TTL);
                    Phase::Egg { id, t: 0.0 }
                }
            }
            Phase::Egg { id, t } => {
                let t = t + dt;
                if t < WINK_AFTER {
                    return Progress::Going(Nest {
                        phase: Phase::Egg { id, t },
                        ..act
                    });
                }
                if let Some(egg) = self.props.get_mut(id) {
                    egg.expire_in(WINK_FADE);
                }
                self.burst(Burst::Sparks, act.center);
                self.perform_quietly(self.character.actions().celebrate);
                return Progress::Finished;
            }
        };
        Progress::Going(Nest { phase, ..act })
    }

    /// Точка кольца на земле: глубина сжата, как у всего, что лежит.
    fn ring_point(&self, center: Vec2, i: usize) -> Vec2 {
        let angle = (i % HEAPS) as f32 * TAU / HEAPS as f32;
        let ring = Vec2::from_angle(angle) * (RING_RADIUS * self.ui);
        center + Vec2::new(ring.x, ring.y * 0.55)
    }
}
