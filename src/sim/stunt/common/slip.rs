//! Кожура: лежит впереди по ходу, персонаж идёт прямо на неё, наступает, подлетает и
//! скользит на спине лапами вверх; кожура улетает назад. Встаёт оглушённым.

use std::f32::consts::PI;

use super::super::Progress;
use crate::animation::tween::Easing;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::{Drive, Sim};

/// Расстояния — логические px, скорости — px/с, время — с.
const TIMEOUT: f32 = 12.0;
const PEEL_AHEAD: f32 = 120.0;
const PEEL_TTL: f32 = 20.0;
/// Идёт на кожуру и дальше: не тормозит перед ней.
const PAST: f32 = 90.0;
const STEP_ON: f32 = 10.0;
const SLIDE_SPEED: f32 = 220.0;
const SLIDE_FRICTION: f32 = 3.0;
const FALL_SECS: f32 = 0.3;
/// Подлёт при падении, design px.
const HOP: f32 = 30.0;
const LIE_SECS: f32 = 1.2;
const RISE_SECS: f32 = 0.5;
const PEEL_KICK: f32 = 200.0;
const PEEL_LIFT: f32 = 300.0;
const PEEL_SPIN: f32 = 14.0;
/// Кожура улетела и гаснет за столько секунд.
const PEEL_GONE: f32 = 3.0;

#[derive(Debug)]
enum Phase {
    Walk,
    Fall(f32),
    Lie(f32),
    Rise(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Slip {
    peel: PropId,
    /// Куда идёт: за кожуру.
    goal: Vec2,
    way: Vec2,
    phase: Phase,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_slip(&mut self) -> Option<Slip> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin();
        let reach = (PEEL_AHEAD + PAST) * self.ui;
        let heading = self.body.heading.normalized();
        let way = if heading == Vec2::ZERO {
            Vec2::new(1.0, 0.0)
        } else {
            heading
        };
        // Идти некуда — разворот: так сценка у края всё равно случается
        let way = [way, -way].into_iter().find(|w| {
            area.clamp(self.body.pos + *w * reach, margin) == self.body.pos + *w * reach
        })?;
        let at = self.body.pos + way * (PEEL_AHEAD * self.ui);
        let peel = self.props.spawn(PropKind::Peel, at, 0.0, self.ui, PEEL_TTL);
        Some(Slip {
            peel,
            goal: self.body.pos + way * reach,
            way,
            phase: Phase::Walk,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_slip(&mut self, mut act: Slip, dt: f32) -> Progress<Slip> {
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let Some(peel) = self.props.get(act.peel).copied() else {
            return Progress::Dropped;
        };
        match act.phase {
            Phase::Walk => {
                self.brain.push(Directive::MoveTo(act.goal));
                if self.body.pos.distance(peel.pos) <= STEP_ON * self.ui {
                    self.halt();
                    let slide = act.way * (SLIDE_SPEED * self.ui);
                    self.drive = Some(Drive::new(slide).friction(SLIDE_FRICTION));
                    if let Some(peel) = self.props.get_mut(act.peel) {
                        peel.vel = -act.way * (PEEL_KICK * self.ui);
                        peel.vz = PEEL_LIFT * self.ui;
                        peel.spin = PEEL_SPIN;
                        peel.expire_in(PEEL_GONE);
                    }
                    self.burst(Burst::Dust, self.body.pos);
                    act.phase = Phase::Fall(0.0);
                }
            }
            Phase::Fall(t) => {
                let u = Easing::OutQuad.apply(t / FALL_SECS);
                self.antic.roll = PI * u;
                self.antic.lift = HOP * (PI * t / FALL_SECS).sin();
                act.phase = if t + dt < FALL_SECS {
                    Phase::Fall(t + dt)
                } else {
                    self.burst(Burst::Dust, self.body.pos);
                    Phase::Lie(LIE_SECS)
                };
            }
            Phase::Lie(left) => {
                self.antic.roll = PI;
                act.phase = if left > dt {
                    Phase::Lie(left - dt)
                } else {
                    // Встаёт на месте
                    self.drive = Some(Drive::new(Vec2::ZERO));
                    Phase::Rise(0.0)
                };
            }
            Phase::Rise(t) => {
                self.antic.roll = PI * (1.0 - Easing::InOutCubic.apply(t / RISE_SECS));
                if t + dt >= RISE_SECS {
                    self.cond.dazed = 1.0;
                    return Progress::Finished;
                }
                act.phase = Phase::Rise(t + dt);
            }
        }
        Progress::Going(act)
    }
}
