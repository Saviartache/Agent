//! Ой: свинка тужится посреди экрана, остаётся кучка с мухами, свинка её закапывает.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Логические px, с.
const CENTER_JITTER: f32 = 80.0;
const ARRIVE: f32 = 24.0;
const WALK_SECS: f32 = 12.0;
const LOOK_SECS: f32 = 1.2;
const LOOK_SWING: f32 = 0.9;
const STRAIN_SECS: f32 = 1.5;
const STRAIN_HZ: f32 = 8.0;
const STEP: f32 = 30.0;
const STEP_SECS: f32 = 1.2;
const GLANCE_SECS: f32 = 0.9;
const BURY_GAP: f32 = 0.3;
const BURY_COUNT: u8 = 3;
const BURY_SECS: f32 = 1.4;
const LEAVE_SECS: f32 = 1.6;
const LEAVE_DISTANCE: [f32; 2] = [160.0, 280.0];
const PILE_TTL: f32 = 20.0;
/// Мухи разлетаются, когда курсор ближе стольких радиусов кучки.
const SHOO_RADII: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk,
    Look,
    Strain,
    Step,
    Glance,
    Bury { dug: u8, gap: f32 },
    Leave,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Oops {
    spot: Vec2,
    phase: Phase,
    t: f32,
    pile: Option<PropId>,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_oops(&mut self) -> Option<Oops> {
        let area = self.bounds.nearest(self.body.pos);
        let jitter = Vec2::new(
            self.rng.range(-CENTER_JITTER, CENTER_JITTER),
            self.rng.range(-CENTER_JITTER, CENTER_JITTER),
        );
        let spot = area.clamp(area.center() + jitter * self.ui, self.edge_margin());
        self.brain.push(Directive::MoveTo(spot));
        Some(Oops {
            spot,
            phase: Phase::Walk,
            t: 0.0,
            pile: None,
        })
    }

    pub(in crate::sim::stunt) fn step_oops(&mut self, mut act: Oops, dt: f32) -> Progress<Oops> {
        act.t += dt;
        self.shoo_flies(act.pile);
        let t = act.t;
        let to = |act: &mut Oops, phase| {
            act.phase = phase;
            act.t = 0.0;
        };
        match act.phase {
            Phase::Walk => {
                if self.body.pos.distance(act.spot) <= ARRIVE * self.ui {
                    self.halt();
                    to(&mut act, Phase::Look);
                } else if t > WALK_SECS {
                    return Progress::Dropped;
                }
            }
            Phase::Look => {
                self.antic.head_yaw = LOOK_SWING * (TAU * 2.0 * t / LOOK_SECS).sin();
                if t >= LOOK_SECS {
                    to(&mut act, Phase::Strain);
                }
            }
            Phase::Strain => {
                let shake = (TAU * STRAIN_HZ * t).sin();
                self.antic.crouch = 4.0;
                self.antic.squash = Vec2::new(1.05 + 0.015 * shake, 0.9);
                self.antic.wobble = 0.5 * shake;
                self.antic.lids = 0.6;
                if t >= STRAIN_SECS {
                    let at = self.body.pos;
                    act.pile = Some(self.props.spawn(PropKind::Pile, at, 0.0, self.ui, PILE_TTL));
                    let ahead = at + self.body.heading * (STEP * self.ui);
                    self.brain.push(Directive::MoveTo(ahead));
                    to(&mut act, Phase::Step);
                }
            }
            Phase::Step => {
                if t >= STEP_SECS {
                    self.halt();
                    to(&mut act, Phase::Glance);
                }
            }
            Phase::Glance => {
                self.antic.head_yaw = 1.0;
                self.antic.neck_side = 0.5;
                self.antic.lids = 0.3;
                if t >= GLANCE_SECS {
                    to(&mut act, Phase::Bury { dug: 0, gap: 0.0 });
                }
            }
            Phase::Bury { dug, gap } => {
                self.antic.wobble = 1.5 * (TAU * 6.0 * t).sin();
                self.antic.crouch = 1.0;
                let (dug, gap) = if gap <= 0.0 && dug < BURY_COUNT {
                    if let Some(pile) = self.pile_at(act.pile) {
                        let mid = (self.body.pos + pile) * 0.5;
                        self.burst(Burst::Dirt, mid);
                    }
                    (dug + 1, BURY_GAP)
                } else {
                    (dug, gap - dt)
                };
                act.phase = Phase::Bury { dug, gap };
                if t >= BURY_SECS {
                    self.burst(Burst::Notes, self.head());
                    let away = self.spot_away(LEAVE_DISTANCE, 0.0, 0.0).unwrap_or(act.spot);
                    self.brain.push(Directive::MoveTo(away));
                    to(&mut act, Phase::Leave);
                }
            }
            Phase::Leave => {
                if t >= LEAVE_SECS {
                    self.halt();
                    return Progress::Finished;
                }
            }
        }
        Progress::Going(act)
    }

    fn pile_at(&self, pile: Option<PropId>) -> Option<Vec2> {
        self.props.get(pile?).map(|prop| prop.pos)
    }

    /// Мухи разлетаются, пока курсор рядом с кучкой.
    fn shoo_flies(&mut self, pile: Option<PropId>) {
        let Some(id) = pile else {
            return;
        };
        let near =
            |cursor: Vec2, pos: Vec2, radius: f32| cursor.distance(pos) <= SHOO_RADII * radius;
        let cursor = self.cursor;
        if let Some(prop) = self.props.get_mut(id) {
            prop.stage = u8::from(cursor.is_some_and(|c| near(c, prop.pos, prop.radius)));
        }
    }
}
