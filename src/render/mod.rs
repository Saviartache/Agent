//! Программная отрисовка: не знает о Windows.

pub mod canvas;
pub mod creature;
mod dachshund;
mod goose;
mod marks;
mod pig;
pub mod pixel;
mod props;
mod puddle;
pub mod scene;
pub mod shadow;

pub use canvas::{Canvas, Color, Sprite};
