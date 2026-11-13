//! Рефлексы: сценки, которые запускает не скука, а то, что случилось рядом
//! (курсор пролетел, грязь подсохла). Проверяются каждый шаг, пока сценки нет.

use crate::behavior::Intent;
use crate::command::Stunt;
use crate::sim::Sim;

/// Курсор быстрее этой доли `startle_speed` персонажа пугает его.
const STARTLE_SPEED: f32 = 0.6;
/// Логические px, с.
const STARTLE_RANGE: f32 = 140.0;
const STARTLE_PAUSE: f32 = 20.0;

impl Sim {
    pub(super) fn reflex(&mut self, dt: f32) {
        if self.act.is_some() || !self.alive() || self.dragging() || !self.settings.stunts {
            return;
        }
        self.startle_reflex();
        self.shake_off_reflex(dt);
    }

    fn startle_reflex(&mut self) {
        // Спящий курсора не замечает
        let Some(cursor) = self.cursor.filter(|_| self.brain.intent() != Intent::Nap) else {
            return;
        };
        let fast = self.character.behavior().startle_speed * STARTLE_SPEED * self.ui;
        if self.clock < self.startle_ready
            || self.cursor_vel.length() <= fast
            || cursor.distance(self.body.pos) > STARTLE_RANGE * self.ui
        {
            return;
        }
        if self.stage(Stunt::Startle) {
            self.startle_ready = self.clock + STARTLE_PAUSE;
        }
    }
}
