//! Строевой шаг: гусь гордо вышагивает через экран, шея прямо, клюв вверх, мерная тряска.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const DISTANCE: [f32; 2] = [400.0, 700.0];
const MIN_DISTANCE: f32 = 300.0;
const ARRIVE: f32 = 14.0;
const LIMIT: f32 = 12.0;
/// Шагов в секунду.
const STEP_HZ: f32 = 2.0;
const STEP_LIFT: f32 = 2.5;
const SWAY: f32 = 0.8;
const NECK_UP: f32 = 6.0;
const HEAD_PITCH: f32 = 0.15;

#[derive(Debug)]
pub(in crate::sim) struct March {
    goal: Vec2,
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_march(&mut self) -> Option<March> {
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
        let reach = self.rng.range(DISTANCE[0], DISTANCE[1]) * self.ui;
        if room < MIN_DISTANCE * self.ui {
            return None;
        }
        let goal = self.body.pos + Vec2::new(side * reach.min(room), 0.0);
        self.brain.push(Directive::MoveTo(goal));
        Some(March { goal, t: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_march(&mut self, mut act: March, dt: f32) -> Progress<March> {
        act.t += dt;
        if self.body.pos.distance(act.goal) <= ARRIVE * self.ui {
            self.halt();
            return Progress::Finished;
        }
        if act.t > LIMIT {
            self.halt();
            return Progress::Dropped;
        }
        let a = &mut self.antic;
        a.neck_up = NECK_UP;
        a.head_pitch = HEAD_PITCH;
        a.lift = STEP_LIFT * (TAU * STEP_HZ * 0.5 * act.t).sin().abs();
        a.wobble = SWAY * (TAU * STEP_HZ * 0.5 * act.t).sin();
        Progress::Going(act)
    }
}
