//! Движение: тело, интегрирование и рулёжка. Без часов и ОС — время приходит как `dt`.

mod body;
pub mod steering;

pub use body::{Arena, Body, Limits, Mover, Steer};
