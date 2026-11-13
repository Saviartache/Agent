//! Записка: персонаж приносит карточку с шуткой своего вида. Только на пустой экран —
//! записка не вытесняет карточки приложения.

use std::time::Duration;

use super::super::Progress;
use crate::command::Notification;
use crate::sim::Sim;

/// Записка лежит меньше карточки приложения.
const NOTE_TTL: Duration = Duration::from_secs(5);
/// Доиграна, когда положена.
const DELIVER_SECS: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Note {
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_note(&mut self) -> Option<Note> {
        let notes = self.notes();
        if !self.items.is_empty() || notes.is_empty() {
            return None;
        }
        let text = notes[self.pick(notes.len())];
        self.notify(Notification::info(text).ttl(NOTE_TTL));
        Some(Note { left: DELIVER_SECS })
    }

    pub(in crate::sim::stunt) fn step_note(&mut self, act: Note, dt: f32) -> Progress<Note> {
        match act.left - dt {
            left if left > 0.0 => Progress::Going(Note { left }),
            _ => Progress::Finished,
        }
    }
}
