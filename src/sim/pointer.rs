//! Нажатие на персонажа: щелчок или перетаскивание.

use super::{Sim, push_event};
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::event::AgentEvent;
use crate::math::Vec2;

/// Курсор должен уйти дальше этого от точки нажатия, чтобы нажатие стало перетаскиванием, логические px.
const DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Down(Vec2),
    Up(Vec2),
    /// Система отобрала захват мыши.
    Cancel,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Press {
    start: Vec2,
    /// Точка нажатия относительно ног.
    pub offset: Vec2,
    pub dragging: bool,
}

impl Sim {
    /// Нажатие вне жизни персонажа (нет на экране, появляется, уходит) игнорируется.
    pub fn pointer(&mut self, event: Pointer) {
        match event {
            Pointer::Down(at) if self.alive() => {
                self.press = Some(Press {
                    start: at,
                    offset: at - self.body.pos,
                    dragging: false,
                });
            }
            Pointer::Up(at) => self.release(at),
            Pointer::Cancel => self.cancel(),
            Pointer::Down(_) => {}
        }
    }

    pub(super) fn dragging(&self) -> bool {
        self.press.is_some_and(|press| press.dragging)
    }

    /// Курсор ушёл от точки нажатия — начинается перетаскивание.
    pub(super) fn begin_drag(&mut self, cursor: Option<Vec2>) {
        let (Some(press), Some(cursor)) = (self.press.as_mut(), cursor) else {
            return;
        };
        let far = cursor.distance(press.start) > DRAG_THRESHOLD * self.ui;
        if press.dragging || !self.settings.draggable || !far {
            return;
        }
        press.dragging = true;
        // Карточка уже не принесена этим действием
        self.silent = None;
        self.cancel_stunt();
        self.brain.push(Directive::DragStart);
    }

    /// Ноги держатся за курсор там, где его взяли.
    pub(super) fn follow_cursor(&mut self, cursor: Option<Vec2>) {
        if let (Some(press), Some(cursor)) = (self.press, cursor) {
            self.body.pos = self.bounds.hull().clamp(cursor - press.offset, 0.0);
        }
        self.body.vel = Vec2::ZERO;
    }

    fn release(&mut self, at: Vec2) {
        let Some(press) = self.press.take() else {
            return;
        };
        if !press.dragging {
            self.brain.push(Directive::Click);
            self.clicked = true;
            push_event(&mut self.events, AgentEvent::Clicked);
            return;
        }
        self.body.pos = self.bounds.hull().clamp(at - press.offset, 0.0);
        self.brain.push(Directive::DragEnd);
        self.burst(Burst::Dust, self.body.pos);
        push_event(&mut self.events, AgentEvent::Dropped(self.body.pos));
    }

    fn cancel(&mut self) {
        if self.press.take().is_some_and(|press| press.dragging) {
            self.brain.push(Directive::DragEnd);
        }
    }
}
