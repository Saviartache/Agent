//! Поведение: небольшой иерархический автомат (жизненный цикл → режим → намерение)
//! и планировщик намерений. Знает действия по индексам, анимаций не видит.

mod brain;
mod grab;
mod intent;
mod scheduler;

pub use brain::{Brain, BrainEvent, CursorSense, Decision, Directive, Permissions, Senses};
pub use grab::CursorGrab;
pub use intent::{Facing, Gait, Intent, Reaction};
pub use scheduler::{IntentKind, Scheduler};
