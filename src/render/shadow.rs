//! Тень точками: пятно из клеток в шахматном порядке, сквозь которое виден рабочий стол.
//! Общая для всего, что стоит или лежит на «земле»: персонажа, листьев, будущих предметов.

use super::{Canvas, Color};
use crate::math::{Rect, Vec2};

/// Непрозрачность клетки под предметом, стоящим на земле.
pub const OPACITY: f32 = 0.34;
const COLOR: Color = Color(0x10_14_1C);
/// Больше этого пятно не рисуется: вход испорчен.
const MAX_REACH: f32 = 2048.0;

/// Эллипс: центр, полуоси и поворот.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oval {
    pub center: Vec2,
    pub radii: Vec2,
    pub rotation: f32,
}

impl Oval {
    pub fn contains(&self, point: Vec2) -> bool {
        let d = point - self.center;
        let (sin, cos) = self.rotation.sin_cos();
        let u = (d.x * cos + d.y * sin) / self.radii.x;
        let v = (d.y * cos - d.x * sin) / self.radii.y;
        u * u + v * v <= 1.0
    }
}

/// Пятно `oval` (в мировых px) клетками со стороной `cell`; `origin` — мировая точка
/// левого верхнего угла холста. Сетка привязана к миру, поэтому пятна разных предметов
/// сливаются в одну шашечку. Звать до остального рисования на холсте: клетка, уже
/// занятая чужой тенью, второй раз не затемняется.
pub fn dotted(canvas: &mut Canvas, origin: Vec2, oval: &Oval, cell: u32, opacity: f32) {
    let reach = oval.radii.x.max(oval.radii.y);
    let sound = reach > 0.0
        && reach < MAX_REACH
        && oval.radii.x.min(oval.radii.y) > 0.0
        && oval.center.x.is_finite()
        && oval.center.y.is_finite()
        && origin.x.is_finite()
        && origin.y.is_finite();
    if !sound || cell == 0 || opacity.is_nan() || opacity <= 0.0 {
        return;
    }
    let side = cell as f32;
    let cells = |center: f32| {
        let first = ((center - reach) / side).floor() as i32;
        first..=((center + reach) / side).ceil() as i32
    };
    for j in cells(oval.center.y) {
        for i in cells(oval.center.x).filter(|i| (i + j) % 2 == 0) {
            let corner = Vec2::new(i as f32, j as f32) * side;
            let min = corner - origin;
            let taken = canvas.alpha_at(min.x.round() as i32, min.y.round() as i32) != 0;
            if !taken && oval.contains(corner + Vec2::new(side, side) * 0.5) {
                let square = Rect::new(min, min + Vec2::new(side, side));
                canvas.fill_round_rect(square, 0.0, COLOR, opacity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIDE: usize = 64;

    fn round(center: Vec2, radius: f32) -> Oval {
        Oval {
            center,
            radii: Vec2::new(radius, radius * 0.6),
            rotation: 0.0,
        }
    }

    fn draw(origin: Vec2, ovals: &[Oval]) -> Vec<u32> {
        let mut pixels = vec![0u32; SIDE * SIDE];
        let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
        for oval in ovals {
            dotted(&mut canvas, origin, oval, 2, OPACITY);
        }
        pixels
    }

    #[test]
    fn spot_is_a_checkerboard_of_whole_cells() {
        let pixels = draw(Vec2::ZERO, &[round(Vec2::new(32.0, 32.0), 20.0)]);
        let alpha = |x: usize, y: usize| pixels[y * SIDE + x] >> 24;
        let filled = pixels.iter().filter(|p| **p >> 24 != 0).count();
        assert!(filled > 100);
        for (x, y) in [(30, 30), (32, 32), (28, 32)] {
            assert_ne!(alpha(x, y), 0, "{x},{y}");
            assert_eq!(alpha(x, y), alpha(x + 1, y + 1));
            assert_eq!(alpha(x + 2, y), 0);
        }
    }

    #[test]
    fn overlapping_spots_do_not_darken_each_other() {
        let one = round(Vec2::new(30.0, 32.0), 16.0);
        let other = round(Vec2::new(36.0, 32.0), 16.0);
        let both = draw(Vec2::ZERO, &[one, other]);
        let single = draw(Vec2::ZERO, &[one]);
        let darkest = |pixels: &[u32]| pixels.iter().map(|p| p >> 24).max().unwrap();
        assert_eq!(darkest(&both), darkest(&single));
        assert!(
            both.iter().filter(|p| **p != 0).count() > single.iter().filter(|p| **p != 0).count()
        );
    }

    #[test]
    fn grid_is_anchored_to_the_world_not_the_canvas() {
        let oval = round(Vec2::new(-470.0, 232.0), 16.0);
        let here = draw(Vec2::new(-502.0, 200.0), &[oval]);
        let shifted = draw(Vec2::new(-503.0, 200.0), &[oval]);
        // Холст сдвинут на пиксель влево — картинка на нём уезжает на пиксель вправо
        assert!((0..SIDE - 1).all(|x| here[32 * SIDE + x] == shifted[32 * SIDE + x + 1]));
    }

    #[test]
    fn broken_input_draws_nothing() {
        let nan = round(Vec2::new(f32::NAN, 0.0), 10.0);
        let flat = Oval {
            radii: Vec2::new(10.0, 0.0),
            ..round(Vec2::new(32.0, 32.0), 10.0)
        };
        let huge = round(Vec2::new(32.0, 32.0), 1e9);
        assert!(draw(Vec2::ZERO, &[nan, flat, huge]).iter().all(|p| *p == 0));
        let mut pixels = vec![0u32; SIDE * SIDE];
        let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
        let good = round(Vec2::new(32.0, 32.0), 10.0);
        dotted(&mut canvas, Vec2::ZERO, &good, 0, OPACITY);
        dotted(&mut canvas, Vec2::ZERO, &good, 2, 0.0);
        dotted(&mut canvas, Vec2::new(f32::NAN, 0.0), &good, 2, OPACITY);
        assert!(pixels.iter().all(|p| *p == 0));
    }
}
