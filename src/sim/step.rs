//! Шаг симуляции: кадр делится на подшаги, в каждом — мозг, движение, анимация, эффекты.

use super::{CURSOR_SMOOTHING, Frame, SUBSTEP_SECS, Sim, clean_dt, finite};
use crate::behavior::{CursorSense, Decision, Gait, Intent, Senses};
use crate::math::Vec2;
use crate::motion::{Arena, Limits};

impl Sim {
    /// `Some(p)` — переставить системный курсор в `p` (удержание).
    pub fn step(&mut self, frame: &Frame) -> Option<Vec2> {
        if !frame.permitted {
            self.grab.release();
            return None;
        }
        if self.paused {
            return None;
        }
        if !self.alive() {
            self.press = None;
        }
        let dt = clean_dt(frame.dt);
        let mut cursor = frame.cursor.filter(|point| finite(*point));
        self.track_cursor(cursor, dt);
        self.begin_drag(cursor);
        let steps = (dt / SUBSTEP_SECS).ceil().max(1.0);
        let sub = dt / steps;
        let mut moved = None;
        for _ in 0..steps as usize {
            moved = self.substep(sub, &mut cursor, frame.user_input).or(moved);
        }
        self.clicked = false;
        if moved.is_some() {
            self.cursor_prev = moved;
        }
        moved
    }

    /// Курсор и его сглаженная скорость; собственное движение при удержании не в счёт.
    fn track_cursor(&mut self, cursor: Option<Vec2>, dt: f32) {
        self.trail.record(cursor, dt);
        let Some(now) = cursor else {
            self.cursor_prev = None;
            self.cursor_vel = Vec2::ZERO;
            return;
        };
        match self.cursor_prev {
            Some(prev) if dt > 0.0 && !self.grab.holding() => {
                let raw = (now - prev) * (1.0 / dt);
                let blend = 1.0 - (-dt * CURSOR_SMOOTHING).exp();
                self.cursor_vel = self.cursor_vel.lerp(raw, blend);
            }
            _ => self.cursor_vel = Vec2::ZERO,
        }
        self.cursor_prev = Some(now);
    }

    fn substep(&mut self, dt: f32, cursor: &mut Option<Vec2>, user_input: bool) -> Option<Vec2> {
        self.clock += dt;
        self.cursor = *cursor;
        let decision = self.think(dt, *cursor);
        if self.brain.intent() != Intent::Absent {
            self.facing = decision.facing;
            self.locomote(&decision, dt, *cursor);
            let speed = self.animation_speed(&decision);
            self.anim
                .update(&self.character, decision.action, speed, dt);
            self.animate_rig(&decision, dt);
        }
        self.advance_stunt(dt);
        self.age_effects(dt);
        self.react();
        self.advance_cards(dt);
        let moved = self.hold_cursor(dt, *cursor, user_input);
        if moved.is_some() {
            *cursor = moved;
        }
        moved
    }

    fn think(&mut self, dt: f32, cursor: Option<Vec2>) -> Decision {
        let senses = Senses {
            pos: self.body.pos,
            cursor: cursor.map(|pos| CursorSense {
                pos,
                vel: self.cursor_vel,
            }),
            arrived: self.arrived,
            action_done: self.anim.done(),
            bounds: &self.bounds,
            scale: self.ui,
            staged: self.act.is_some(),
        };
        let decision = self.brain.update(dt, &senses);
        self.brain.drain_events(&mut self.brain_events);
        if self.brain.intent() != Intent::Absent {
            self.spawn_pending = false;
        }
        decision
    }

    fn gait_speed(&self, gait: Gait) -> f32 {
        let motion = self.character.motion();
        let speed = match gait {
            Gait::Walk => motion.walk_speed,
            Gait::Run => motion.run_speed,
        };
        speed * self.ui
    }

    fn locomote(&mut self, decision: &Decision, dt: f32, cursor: Option<Vec2>) {
        self.bumped = None;
        if self.dragging() {
            self.follow_cursor(cursor);
            self.arrived = false;
            return;
        }
        if self.drive.is_some() {
            self.drive_body(dt);
            self.arrived = false;
            return;
        }
        let motion = self.character.motion();
        let limits = Limits {
            max_speed: self.gait_speed(decision.gait),
            acceleration: motion.acceleration * self.ui,
            deceleration: motion.deceleration * self.ui,
        };
        let arena = Arena {
            bounds: &self.bounds,
            margin: self.edge_margin(),
        };
        self.arrived = self.mover.step(
            &mut self.body,
            &decision.steer,
            &limits,
            &arena,
            &mut self.rng,
            dt,
        );
    }

    /// Шаг анимации ходьбы следует за фактической скоростью, чтобы ноги не скользили.
    fn animation_speed(&self, decision: &Decision) -> f32 {
        let actions = self.character.actions();
        let nominal = self.gait_speed(decision.gait);
        let walking = decision.action == actions.walk || decision.action == actions.run;
        if walking && nominal > 0.0 {
            (self.body.vel.length() / nominal).clamp(0.6, 1.4)
        } else {
            1.0
        }
    }

    /// Курсор — в клюве или у рыла. Предохранитель сам решает, можно ли держать;
    /// без разрешения в настройках он не вызывается.
    fn hold_cursor(&mut self, dt: f32, cursor: Option<Vec2>, user_input: bool) -> Option<Vec2> {
        if !self.settings.grab {
            return None;
        }
        let head = self.head();
        self.grab.step(dt, cursor, head, user_input, true)
    }
}
