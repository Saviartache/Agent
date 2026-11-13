//! Эффекты: следы, частицы, предметы сценок (листья, лужи) и принесённые карточки.
//! Чистые модели без часов и отрисовки.

pub mod footprints;
pub mod items;
pub mod leaves;
pub mod mud;
pub mod particles;
pub mod props;

use crate::math::{Rect, Vec2};

/// Отрицательный и NaN шаг — ноль.
fn sane_dt(dt: f32) -> f32 {
    if dt > 0.0 { dt } else { 0.0 }
}

/// Охватывающий прямоугольник кругов `(центр, радиус)`.
fn bounding(circles: impl Iterator<Item = (Vec2, f32)>) -> Option<Rect> {
    circles
        .map(|(center, radius)| Rect::new(center, center).inflate(radius))
        .reduce(Rect::union)
}
