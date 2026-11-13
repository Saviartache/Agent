//! Общий «пиксельный» стиль: всё на рабочем столе рисуется лёгкой мозаикой на одной сетке.

use super::Canvas;
use crate::math::Vec2;

/// Сторона «пикселя» мозаики, логические px.
const PIXEL: f32 = 1.4;
/// Насколько пиксели внутри квадрата сведены к его среднему: меньше — мягче рябь.
const STRENGTH: f32 = 0.55;

/// Сторона клетки мозаики и тени при этом масштабе, px.
pub fn cell(scale: f32) -> u32 {
    if scale.is_finite() {
        (PIXEL * scale).round().clamp(2.0, 64.0) as u32
    } else {
        2
    }
}

/// Мозаика поверх готового холста. Сетка привязана к миру (`origin` — мировая точка
/// левого верхнего угла холста): на ходу картинка рябит, как нарисованная в низком разрешении.
pub fn pixelate(canvas: &mut Canvas, origin: Vec2, scale: f32) {
    if !(origin.x.is_finite() && origin.y.is_finite()) {
        return;
    }
    let cell = cell(scale);
    canvas.pixelate(cell, phase(origin, cell), STRENGTH);
}

/// Ретро вместо мозаики для того, что лежит на земле: целые клетки той же мировой сетки,
/// одним цветом, край — шашечкой в лад с тенью.
pub fn retro(canvas: &mut Canvas, origin: Vec2, scale: f32) {
    if !(origin.x.is_finite() && origin.y.is_finite()) {
        return;
    }
    let cell = cell(scale);
    let index = |at: f32| (at.round() as i64).div_euclid(i64::from(cell));
    let odd = (index(origin.x) + index(origin.y)).rem_euclid(2) == 1;
    canvas.retro(cell, phase(origin, cell), odd);
}

fn phase(origin: Vec2, cell: u32) -> [u32; 2] {
    let phase = |at: f32| (at.round() as i64).rem_euclid(i64::from(cell)) as u32;
    [phase(origin.x), phase(origin.y)]
}
