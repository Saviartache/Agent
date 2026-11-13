//! Тявканье: такса подходит к курсору мордой, припадает на передние лапы и тявкает
//! с подскоком, виляя хвостом. Курсор ушёл — довольна, что прогнала.

use std::f32::consts::PI;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const NOTICE_RANGE: f32 = 450.0;
/// Стоит на таком расстоянии от курсора; подходит с разбега, чтобы встать к нему мордой.
const STAND_OFF: f32 = 90.0;
const RUN_UP: f32 = 40.0;
const ARRIVE: f32 = 10.0;
const WALK_SECS: f32 = 8.0;
const YAPS: [usize; 2] = [4, 7];
const YAP_GAP: f32 = 0.5;
const YAP_OPEN: f32 = 0.18;
const YAP_HOP: f32 = 4.0;
/// Поклон: перед ниже зада.
const BOW_PITCH: f32 = -0.15;
const BOW_CROUCH: f32 = 1.5;

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// Отходит к началу разбега, потом подходит к месту.
    Approach {
        spot: Vec2,
        run_up: bool,
        left: f32,
    },
    Yap(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Yap {
    phase: Phase,
    yaps: usize,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_yap(&mut self) -> Option<Yap> {
        let cursor = self.cursor?;
        if cursor.distance(self.body.pos) > NOTICE_RANGE * self.ui {
            return None;
        }
        let spot = self.yap_spot(cursor, RUN_UP);
        self.brain.push(Directive::MoveTo(spot));
        let yaps = YAPS[0] + self.pick(YAPS[1] - YAPS[0] + 1);
        Some(Yap {
            phase: Phase::Approach {
                spot,
                run_up: true,
                left: WALK_SECS,
            },
            yaps,
        })
    }

    pub(in crate::sim::stunt) fn step_yap(&mut self, mut act: Yap, dt: f32) -> Progress<Yap> {
        let Some(cursor) = self.cursor else {
            return self.yap_done(&act);
        };
        act.phase = match act.phase {
            Phase::Approach { left, .. } if left <= dt => return Progress::Dropped,
            Phase::Approach { spot, run_up, left } => {
                if self.body.pos.distance(spot) > ARRIVE * self.ui {
                    Phase::Approach {
                        spot,
                        run_up,
                        left: left - dt,
                    }
                } else if run_up {
                    let spot = self.yap_spot(cursor, 0.0);
                    self.brain.push(Directive::MoveTo(spot));
                    Phase::Approach {
                        spot,
                        run_up: false,
                        left: left - dt,
                    }
                } else {
                    self.halt();
                    Phase::Yap(0.0)
                }
            }
            Phase::Yap(t) => {
                let done = (t / YAP_GAP) as usize;
                if done >= act.yaps {
                    return self.yap_done(&act);
                }
                let local = t - done as f32 * YAP_GAP;
                if local < dt {
                    self.burst(Burst::Rings, self.head());
                }
                let open = local < YAP_OPEN;
                self.antic.mouth = if open { 1.0 } else { 0.0 };
                self.antic.lift = YAP_HOP * (PI * (local / YAP_OPEN).min(1.0)).sin();
                self.antic.pitch = BOW_PITCH;
                self.antic.crouch = BOW_CROUCH;
                self.antic.neck_up = 2.0;
                self.antic.head_pitch = 0.3;
                self.antic.wag = 1.0;
                Phase::Yap(t + dt)
            }
        };
        Progress::Going(act)
    }

    /// Место сбоку от курсора, со стороны таксы; `back` — запас на разбег.
    fn yap_spot(&self, cursor: Vec2, back: f32) -> Vec2 {
        let side = if self.body.pos.x < cursor.x {
            -1.0
        } else {
            1.0
        };
        let at = cursor + Vec2::new(side * (STAND_OFF + back) * self.ui, 0.0);
        self.bounds.nearest(cursor).clamp(at, self.edge_margin())
    }

    /// Хоть раз тявкнула — доиграно.
    fn yap_done(&mut self, act: &Yap) -> Progress<Yap> {
        self.halt();
        match act.phase {
            Phase::Yap(t) if t > 0.0 => Progress::Finished,
            _ => Progress::Dropped,
        }
    }
}
