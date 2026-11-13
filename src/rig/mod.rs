//! Процедурное тело персонажа: поза считается каждый кадр из движения и жеста,
//! а не берётся из листа спрайтов. Чистое ядро: ни ОС, ни часов.

mod creature;
mod vec3;

pub use creature::{
    Antic, Body, Creature, Ear, Eyes, Footfall, GROUND_SQUASH, Head, Look, NECK_POINTS, Pose,
    RigInput, flatten,
};
pub use vec3::{Frame, Vec3};

use crate::action::names;

/// Какое тело рисует движок; значение поля `"rig"` манифеста.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RigKind {
    #[default]
    Goose,
    Pig,
    Elephant,
    Raccoon,
    Penguin,
}

impl RigKind {
    pub const ALL: [Self; 5] = [
        Self::Goose,
        Self::Pig,
        Self::Elephant,
        Self::Raccoon,
        Self::Penguin,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Goose => "goose",
            Self::Pig => "pig",
            Self::Elephant => "elephant",
            Self::Raccoon => "raccoon",
            Self::Penguin => "penguin",
        }
    }
}

/// Что делает тело; выводится из имени действия, показанного сейчас.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Idle,
    Walk,
    Run,
    Look,
    React,
    Interact,
    Celebrate,
    Think,
    Busy,
    Warning,
    Error,
    Sleep,
    WakeUp,
    Dragged,
    Spawn,
    Despawn,
    Wave,
}

impl Gesture {
    /// Встроенные имена действий; `turn` у тела не нужен, а любое чужое имя
    /// (своё действие набора) получает общее покачивание головой.
    pub fn from_action(name: &str) -> Self {
        match name {
            names::IDLE | names::TURN => Self::Idle,
            names::WALK => Self::Walk,
            names::RUN => Self::Run,
            names::LOOK => Self::Look,
            names::REACT => Self::React,
            names::INTERACT => Self::Interact,
            names::CELEBRATE => Self::Celebrate,
            names::THINK => Self::Think,
            names::BUSY => Self::Busy,
            names::WARNING => Self::Warning,
            names::ERROR => Self::Error,
            names::SLEEP => Self::Sleep,
            names::WAKE_UP => Self::WakeUp,
            names::DRAGGED => Self::Dragged,
            names::SPAWN => Self::Spawn,
            names::DESPAWN => Self::Despawn,
            _ => Self::Wave,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_names_map_and_unknown_waves() {
        assert_eq!(Gesture::from_action("walk"), Gesture::Walk);
        assert_eq!(Gesture::from_action("wake_up"), Gesture::WakeUp);
        assert_eq!(Gesture::from_action("wave"), Gesture::Wave);
        assert_eq!(Gesture::from_action("dance"), Gesture::Wave);
        assert_eq!(RigKind::from_name("goose"), Some(RigKind::Goose));
        assert_eq!(RigKind::from_name("pig"), Some(RigKind::Pig));
        assert_eq!(RigKind::from_name("elephant"), Some(RigKind::Elephant));
        assert_eq!(RigKind::from_name("raccoon"), Some(RigKind::Raccoon));
        assert_eq!(RigKind::from_name("penguin"), Some(RigKind::Penguin));
        assert_eq!(RigKind::from_name("swan"), None);
    }
}
