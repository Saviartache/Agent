//! Программная отрисовка: не знает о Windows.

pub mod canvas;
pub mod creature;
mod marks;
pub mod pixel;
mod props;
mod puddle;
mod puppet;
pub mod scene;
pub mod shadow;
mod solid;

pub use canvas::{Canvas, Color, Sprite};
pub use puppet::{Figure, Puppet};
pub use solid::Solid;
