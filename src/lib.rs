//! Персонаж рабочего стола Windows.
//!
//! Ядро (`animation`, `assets`, `behavior`, `motion`, `effects`, `render`, `rig`, `sim`) не знает
//! о Windows и проверяется тестами на любой машине; окна, курсор и мониторы живут
//! в `platform`, поток персонажа — в `runtime`, и собираются они только под Windows.

pub mod action;
pub mod animation;
pub mod assets;
pub mod behavior;
pub mod command;
pub mod companion;
pub mod config;
pub mod effects;
pub mod error;
pub mod event;
pub mod math;
pub mod motion;
pub mod render;
pub mod rig;
pub mod rng;
pub mod sim;

#[cfg(windows)]
mod agent;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod runtime;

#[cfg(windows)]
pub use agent::{Agent, AgentHandle};
pub use assets::Character;
pub use command::{BehaviorMode, Command, Notification, Status, Stunt, Tone};
pub use companion::{Activity, Companion};
pub use config::{AgentConfig, CursorGrabConfig};
pub use error::AgentError;
pub use event::AgentEvent;
pub use math::Vec2;
