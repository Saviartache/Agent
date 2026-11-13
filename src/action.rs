//! Действие — то, что персонаж делает сейчас. Набор действий задаёт манифест;
//! поведение знает лишь индексы и не знает ни имён, ни анимаций.

/// Индекс действия в таблице персонажа.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub u16);

/// Индекс клипа в таблице персонажа.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClipId(pub u16);

/// Имена действий, которые движок запускает сам.
pub mod names {
    pub const SPAWN: &str = "spawn";
    pub const IDLE: &str = "idle";
    pub const WALK: &str = "walk";
    pub const RUN: &str = "run";
    pub const TURN: &str = "turn";
    pub const LOOK: &str = "look";
    pub const REACT: &str = "react";
    pub const INTERACT: &str = "interact";
    pub const CELEBRATE: &str = "celebrate";
    pub const THINK: &str = "think";
    pub const BUSY: &str = "busy";
    pub const WARNING: &str = "warning";
    pub const ERROR: &str = "error";
    pub const SLEEP: &str = "sleep";
    pub const WAKE_UP: &str = "wake_up";
    pub const DRAGGED: &str = "dragged";
    pub const DESPAWN: &str = "despawn";
}

/// Встроенные действия, разрешённые в индексы конкретного персонажа.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionSet {
    pub spawn: ActionId,
    pub idle: ActionId,
    pub walk: ActionId,
    pub run: ActionId,
    pub turn: Option<ActionId>,
    pub look: ActionId,
    pub react: ActionId,
    pub interact: ActionId,
    pub celebrate: ActionId,
    pub think: ActionId,
    pub busy: ActionId,
    pub warning: ActionId,
    pub error: ActionId,
    pub sleep: ActionId,
    pub wake_up: Option<ActionId>,
    pub dragged: ActionId,
    pub despawn: Option<ActionId>,
}

impl ActionSet {
    /// Обязательны только `idle` и `walk`; остальное заменяется ближайшим по смыслу.
    /// Переходные действия (`turn`, `wake_up`, `despawn`) без замены пропускаются.
    pub fn resolve(find: impl Fn(&str) -> Option<ActionId>) -> Option<Self> {
        let idle = find(names::IDLE)?;
        let walk = find(names::WALK)?;
        let or = |name: &str, fallback: ActionId| find(name).unwrap_or(fallback);
        let react = or(names::REACT, idle);
        let think = or(names::THINK, idle);
        Some(Self {
            spawn: or(names::SPAWN, idle),
            idle,
            walk,
            run: or(names::RUN, walk),
            turn: find(names::TURN),
            look: or(names::LOOK, idle),
            react,
            interact: or(names::INTERACT, react),
            celebrate: or(names::CELEBRATE, react),
            think,
            busy: or(names::BUSY, think),
            warning: or(names::WARNING, react),
            error: or(names::ERROR, react),
            sleep: or(names::SLEEP, idle),
            wake_up: find(names::WAKE_UP),
            dragged: or(names::DRAGGED, react),
            despawn: find(names::DESPAWN),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_actions_fall_back() {
        let set = ActionSet::resolve(|name| match name {
            names::IDLE => Some(ActionId(0)),
            names::WALK => Some(ActionId(1)),
            names::REACT => Some(ActionId(2)),
            _ => None,
        })
        .unwrap();
        assert_eq!(set.run, ActionId(1));
        assert_eq!(set.celebrate, ActionId(2));
        assert_eq!(set.busy, ActionId(0));
        assert_eq!(set.turn, None);
    }

    #[test]
    fn idle_and_walk_are_required() {
        assert!(ActionSet::resolve(|name| (name == names::IDLE).then_some(ActionId(0))).is_none());
    }
}
