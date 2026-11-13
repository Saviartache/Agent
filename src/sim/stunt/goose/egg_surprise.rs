//! Яйцо-сюрприз: гусь тужится, делает шаг и оборачивается — позади яйцо. Оно трескается,
//! и из него выкатывается стрелочка курсора. Настоящий курсор не трогаем.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const STRAIN_SECS: f32 = 1.5;
const STEP_DISTANCE: f32 = 30.0;
const STEP_SECS: f32 = 3.0;
/// Яйцо ложится под хвостом, design px.
const LAY_BEHIND: f32 = 12.0;
const EGG_TTL: f32 = 12.0;
const TURN_SECS: f32 = 0.5;
const HOP_SECS: f32 = 0.4;
const HOP_HEIGHT: f32 = 14.0;
/// Столько стоит ошарашенный, прежде чем яйцо начнёт трескаться.
const SHOCK_SECS: f32 = 0.9;
const CRACK_GAP: f32 = 0.5;
const CRACKS: u8 = 3;
const ARROW_SPEED: f32 = 60.0;
const ARROW_SPIN: f32 = 5.0;
const ARROW_TTL: f32 = 6.0;
const GAZE_SECS: f32 = 1.6;
const SETTLE_SECS: f32 = 0.4;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Strain(f32),
    Step {
        to: Vec2,
        left: f32,
    },
    /// Оборот и прыжок; потом стоит ошарашенный.
    Shock(f32),
    Crack {
        t: f32,
        stage: u8,
    },
    Gaze(f32),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct EggSurprise {
    egg: Option<PropId>,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_egg_surprise(&mut self) -> Option<EggSurprise> {
        self.halt();
        Some(EggSurprise {
            egg: None,
            phase: Phase::Strain(0.0),
        })
    }

    pub(in crate::sim::stunt) fn step_egg_surprise(
        &mut self,
        mut act: EggSurprise,
        dt: f32,
    ) -> Progress<EggSurprise> {
        act.phase = match act.phase {
            Phase::Strain(t) => {
                let t = t + dt;
                if t < STRAIN_SECS {
                    self.antic.crouch = 4.0;
                    self.antic.squash = Vec2::new(1.0, 0.9);
                    self.antic.lids = 0.7;
                    self.antic.wobble = 0.6 * (t * TAU * 14.0).sin();
                    Phase::Strain(t)
                } else {
                    let behind = self.body.pos - self.body.heading * (LAY_BEHIND * self.sprite);
                    act.egg = Some(
                        self.props
                            .spawn(PropKind::Egg, behind, 0.0, self.ui, EGG_TTL),
                    );
                    let to = self.body.pos + self.body.heading * (STEP_DISTANCE * self.ui);
                    let to = self.bounds.clamp(to, self.edge_margin());
                    self.brain.push(Directive::MoveTo(to));
                    Phase::Step {
                        to,
                        left: STEP_SECS,
                    }
                }
            }
            Phase::Step { to, left } => {
                if self.body.pos.distance(to) <= 6.0 * self.ui || left <= dt {
                    self.halt();
                    self.burst(Burst::Alert, self.head());
                    Phase::Shock(0.0)
                } else {
                    Phase::Step {
                        to,
                        left: left - dt,
                    }
                }
            }
            Phase::Shock(t) => {
                let t = t + dt;
                self.shock_pose(t);
                if t < SHOCK_SECS {
                    Phase::Shock(t)
                } else {
                    Phase::Crack { t: 0.0, stage: 0 }
                }
            }
            Phase::Crack { t, stage } => {
                let t = t + dt;
                self.shock_pose(SHOCK_SECS);
                let Some(id) = act.egg else {
                    return Progress::Dropped;
                };
                let at = self.props.get(id).map(|egg| egg.pos);
                let Some(at) = at else {
                    return Progress::Dropped;
                };
                let mut stage = stage;
                let mut t = t;
                if t >= CRACK_GAP {
                    t = 0.0;
                    stage += 1;
                    if stage <= CRACKS {
                        self.burst(Burst::Dust, at);
                    }
                }
                if let Some(egg) = self.props.get_mut(id) {
                    egg.stage = stage.min(CRACKS);
                    // Покачивание нарастает с каждой трещиной
                    egg.angle = 0.12 * stage as f32 * (t * TAU * 6.0).sin();
                }
                if stage > CRACKS {
                    self.hatch(id, at);
                    Phase::Gaze(0.0)
                } else {
                    Phase::Crack { t, stage }
                }
            }
            Phase::Gaze(t) => {
                let t = t + dt;
                if t >= GAZE_SECS {
                    return Progress::Finished;
                }
                // К концу разворачивается обратно
                let back = ((t - (GAZE_SECS - SETTLE_SECS)) / SETTLE_SECS).clamp(0.0, 1.0);
                self.antic.spin = PI * (1.0 - back);
                self.antic.head_pitch = -0.3;
                Phase::Gaze(t)
            }
        };
        Progress::Going(act)
    }

    /// Оборот к яйцу, расширенные глаза, прыжок от испуга.
    fn shock_pose(&mut self, t: f32) {
        self.antic.spin = PI * (t / TURN_SECS).min(1.0);
        self.antic.eyes = Eyes::Wide;
        if t < HOP_SECS {
            self.antic.lift = HOP_HEIGHT * (t / HOP_SECS * PI).sin();
        }
    }

    /// Яйцо гаснет, на его месте выкатывается стрелочка.
    fn hatch(&mut self, id: PropId, at: Vec2) {
        if let Some(egg) = self.props.get_mut(id) {
            egg.expire_in(0.2);
        }
        self.burst(Burst::Puff, at);
        let arrow = self
            .props
            .spawn(PropKind::Arrow, at, 0.0, self.ui, ARROW_TTL);
        let away = (at - self.body.pos).normalized();
        let side = Vec2::new(-away.y, away.x) * 0.6;
        if let Some(arrow) = self.props.get_mut(arrow) {
            arrow.vel = (away + side).normalized() * (ARROW_SPEED * self.ui);
            arrow.spin = ARROW_SPIN;
        }
    }
}
