//! Важное шествие: пингвин идёт через экран, грудь вперёд, ласты чуть врозь,
//! кивает на каждый шаг и вразвалку сильнее обычного.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с; амплитуды — design px и радианы.
const DISTANCE: [f32; 2] = [300.0, 550.0];
const MIN_DISTANCE: f32 = 220.0;
const ARRIVE: f32 = 14.0;
const LIMIT: f32 = 14.0;
/// Кивков в секунду.
const NOD_HZ: f32 = 1.6;
const NOD: f32 = 0.35;
const SWAY: f32 = 1.5;
const CHEST: f32 = 0.12;
const WINGS: f32 = 0.25;

#[derive(Debug)]
pub(in crate::sim) struct Strut {
    goal: Vec2,
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_strut(&mut self) -> Option<Strut> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin() * 2.0;
        let (left, right) = (
            self.body.pos.x - area.min.x - margin,
            area.max.x - self.body.pos.x - margin,
        );
        let (side, room) = if left > right {
            (-1.0, left)
        } else {
            (1.0, right)
        };
        if room < MIN_DISTANCE * self.ui {
            return None;
        }
        let reach = self.rng.range(DISTANCE[0], DISTANCE[1]) * self.ui;
        let goal = self.body.pos + Vec2::new(side * reach.min(room), 0.0);
        self.brain.push(Directive::MoveTo(goal));
        Some(Strut { goal, t: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_strut(&mut self, mut act: Strut, dt: f32) -> Progress<Strut> {
        act.t += dt;
        if self.body.pos.distance(act.goal) <= ARRIVE * self.ui {
            self.halt();
            return Progress::Finished;
        }
        if act.t > LIMIT {
            self.halt();
            return Progress::Dropped;
        }
        let phase = TAU * NOD_HZ * act.t;
        let a = &mut self.antic;
        a.head_pitch = NOD * phase.sin().min(0.0);
        a.pitch = CHEST;
        a.wobble = SWAY * (phase * 0.5).sin();
        a.wings = WINGS;
        Progress::Going(act)
    }
}
