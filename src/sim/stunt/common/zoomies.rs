//! Носится кругами: два круга бегом рядом с местом, где стоял, потом голова кругом —
//! звёздочки и встряска.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Логические px, рад, с.
const RADIUS: f32 = 90.0;
const LAPS: f32 = 2.0;
/// Цель — на столько впереди по кругу: бежит не тормозя, срезая дугу.
const LEAD: f32 = 0.9;
/// Не обежал за это время (упёрся в край) — сценка сорвалась.
const SECS: f32 = 12.0;
const DIZZY_SECS: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Zoomies {
    center: Vec2,
    /// По часовой или против.
    spin: f32,
    /// Пройденный угол вокруг середины.
    swept: f32,
    last: f32,
    left: f32,
    dizzy: Option<f32>,
}

/// Разность углов в `(-π, π]`.
fn wrapped(delta: f32) -> f32 {
    (delta + PI).rem_euclid(TAU) - PI
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_zoomies(&mut self) -> Option<Zoomies> {
        let area = self.bounds.nearest(self.body.pos);
        let radius = RADIUS * self.ui;
        let side = Vec2::from_angle(self.rng.range(0.0, TAU));
        let center = area.clamp(self.body.pos + side * radius, self.edge_margin() + radius);
        let spin = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
        let act = Zoomies {
            center,
            spin,
            swept: 0.0,
            last: self.angle_around(center),
            left: SECS,
            dizzy: None,
        };
        self.lead(&act);
        Some(act)
    }

    pub(in crate::sim::stunt) fn step_zoomies(
        &mut self,
        mut act: Zoomies,
        dt: f32,
    ) -> Progress<Zoomies> {
        if let Some(dizzy) = act.dizzy {
            act.dizzy = Some(dizzy - dt).filter(|left| *left > 0.0);
            return match act.dizzy {
                Some(_) => Progress::Going(act),
                None => Progress::Finished,
            };
        }
        act.left -= dt;
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        let now = self.angle_around(act.center);
        // Назад по кругу не в счёт: только пройденное вперёд
        act.swept += (wrapped(now - act.last) * act.spin).max(0.0);
        act.last = now;
        if act.swept >= LAPS * TAU {
            self.halt();
            self.burst(Burst::Stars, self.head());
            self.perform_quietly(self.character.actions().react);
            act.dizzy = Some(DIZZY_SECS);
            return Progress::Going(act);
        }
        self.lead(&act);
        Progress::Going(act)
    }

    fn angle_around(&self, center: Vec2) -> f32 {
        let from = self.body.pos - center;
        from.y.atan2(from.x)
    }

    fn lead(&mut self, act: &Zoomies) {
        let angle = act.last + LEAD * act.spin;
        let target = act.center + Vec2::from_angle(angle) * (RADIUS * self.ui);
        let target = self.bounds.clamp(target, self.edge_margin());
        self.brain.push(Directive::RunTo(target));
    }
}
