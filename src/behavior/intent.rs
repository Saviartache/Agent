//! Намерение — чего персонаж хочет; живёт секундами. Действие и анимация — уровнем ниже.

use crate::action::ActionId;
use crate::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    Absent,
    Spawning,
    Wander,
    Rest,
    ObserveCursor,
    ApproachCursor,
    ChaseCursor,
    FleeCursor,
    Nap,
    Waking,
    Stay,
    Follow,
    GoTo(Vec2),
    Work,
    Perform(ActionId),
    Dragged,
    Despawning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Walk,
    Run,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

/// Эмоциональный отклик, о котором сообщается наружу (звук, частицы).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Click,
    Contact,
    Startle,
    Joy,
    Alarm,
    Failure,
}
