//! Кража курсора: персонаж подбегает к курсору, хватает его клювом или рылом и носится
//! с ним зигзагами несколько секунд, потом бросает и радуется. Держит курсор только
//! предохранитель `CursorGrab`: без `cursor_grab.enabled` сценка не начинается, а кнопка
//! мыши, Esc или рывок курсора назад отнимают его сразу.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const STALK_SECS: f32 = 8.0;
/// Курсор ушёл от прошлой цели дальше этого — бежим за ним заново.
const RETARGET: f32 = 12.0;
/// Курсор у клюва — хватает.
const REACH: f32 = 22.0;
/// Пробежка с добычей: куда и как долго, прежде чем свернуть.
const DASH: [f32; 2] = [200.0, 420.0];
const DASH_MIN: f32 = 120.0;
const DASH_SECS: f32 = 1.3;
const TURN_AT: f32 = 60.0;
/// Бросает раньше, чем отпустил бы предохранитель: срок кончился — значит, доиграно.
const LET_GO_EARLY: f32 = 0.1;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) enum CursorHeist {
    Stalk { left: f32, aim: Vec2 },
    Carry { left: f32, dash: f32, goal: Vec2 },
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_cursor_heist(&mut self) -> Option<CursorHeist> {
        let cursor = self.cursor?;
        if !self.settings.grab || !self.grab.ready() {
            return None;
        }
        self.run_after(cursor);
        Some(CursorHeist::Stalk {
            left: STALK_SECS,
            aim: cursor,
        })
    }

    pub(in crate::sim::stunt) fn step_cursor_heist(
        &mut self,
        act: CursorHeist,
        dt: f32,
    ) -> Progress<CursorHeist> {
        match act {
            CursorHeist::Stalk { left, aim } => self.stalk(left - dt, aim, self.cursor),
            CursorHeist::Carry { left, dash, goal } => self.carry(left - dt, dash - dt, goal),
        }
    }

    fn stalk(&mut self, left: f32, aim: Vec2, cursor: Option<Vec2>) -> Progress<CursorHeist> {
        let Some(cursor) = cursor.filter(|_| left > 0.0) else {
            return Progress::Dropped;
        };
        let contact = self.character.behavior().contact_radius * self.ui;
        let caught = self.head().distance(cursor) <= REACH * self.ui
            || self.body.pos.distance(cursor) <= contact;
        if caught {
            if !self.grab.try_begin(cursor) {
                return Progress::Dropped;
            }
            self.burst(Burst::Sparks, self.head());
            return Progress::Going(CursorHeist::Carry {
                left: (self.grab.limit() - LET_GO_EARLY).max(0.0),
                dash: 0.0,
                goal: self.body.pos,
            });
        }
        let aim = if aim.distance(cursor) > RETARGET * self.ui {
            self.run_after(cursor);
            cursor
        } else {
            aim
        };
        Progress::Going(CursorHeist::Stalk { left, aim })
    }

    fn carry(&mut self, left: f32, dash: f32, goal: Vec2) -> Progress<CursorHeist> {
        if !self.grab.holding() {
            // Отняли: кнопка, Esc или рывок назад
            self.halt();
            self.perform_quietly(self.character.actions().react);
            return Progress::Dropped;
        }
        if left <= 0.0 {
            self.grab.release();
            self.halt();
            self.perform_quietly(self.character.actions().celebrate);
            return Progress::Finished;
        }
        let turn = dash <= 0.0 || self.body.pos.distance(goal) <= TURN_AT * self.ui;
        let (dash, goal) = match turn.then(|| self.spot_away(DASH, DASH_MIN, 0.0)).flatten() {
            Some(next) => {
                self.brain.push(Directive::RunTo(next));
                (DASH_SECS, next)
            }
            None => (dash, goal),
        };
        Progress::Going(CursorHeist::Carry { left, dash, goal })
    }

    fn run_after(&mut self, cursor: Vec2) {
        let target = self.bounds.clamp(cursor, self.edge_margin());
        self.brain.push(Directive::RunTo(target));
    }
}
