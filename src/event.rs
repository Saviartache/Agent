//! События персонажа приложению.

use crate::command::Stunt;
use crate::math::Vec2;

#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    Spawned,
    Despawned,
    Clicked,
    /// Пользователь перенёс персонажа и отпустил.
    Dropped(Vec2),
    /// Разовое действие, запущенное `Command::Play`, доиграно.
    Played(String),
    /// Сценка доиграна до конца (прерванная событием не отмечается).
    StuntFinished(Stunt),
    /// Карточка ушла: по сроку или по щелчку пользователя.
    NotificationClosed {
        by_user: bool,
    },
    /// Пользователь выбрал в меню «Пауза»/«Продолжить».
    PauseToggled(bool),
    /// Пользователь выбрал в меню «Скрыть»; вернуть — `Command::SetVisible(true)`.
    HiddenByUser,
    /// Пользователь выбрал в меню «Выход»; останавливать ли поток, решает приложение.
    ExitRequested,
    /// Поток персонажа завершён.
    Stopped,
}
