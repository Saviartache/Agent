//! Лужа клетками мировой сетки, как тень: кайма и переходы тонов — шашечкой, без мягких краёв.

use super::{Canvas, Color};
use crate::effects::mud::{LOBES, Liquid, Mud, Puddle};
use crate::math::{Rect, Vec2};

/// Край, сама лужа и блик.
pub(super) const MUD_TONES: [Color; 3] = [Color(0x57_38_1F), Color(0x6E_4A_2A), Color(0x8C_68_44)];
const WATER_TONES: [Color; 3] = [Color(0x4F_86_BF), Color(0x7D_B3_E8), Color(0xCD_E6_FA)];
/// Пороги глубины клетки (1 — край доли, 0 — её середина), от края к середине:
/// край шашечкой, край сплошной, край шашечкой поверх лужи.
const FRINGE: f32 = 0.88;
const RIM: f32 = 0.74;
const SHORE: f32 = 0.62;
const GLOSS_AT: Vec2 = Vec2::new(-0.3, -0.3);
const GLOSS_SIZE: f32 = 0.42;
/// Сплошная середина блика, остальное — шашечкой.
const GLOSS_CORE: f32 = 0.5;

/// `cell` — сторона клетки тени; сетка общая, поэтому лужа и тени ложатся одной шашечкой.
pub(super) fn draw(canvas: &mut Canvas, origin: Vec2, mud: &Mud, cell: u32) {
    if cell == 0 || !(origin.x.is_finite() && origin.y.is_finite()) {
        return;
    }
    for puddle in mud.iter() {
        draw_puddle(canvas, origin, &puddle, cell as f32);
    }
}

fn draw_puddle(canvas: &mut Canvas, origin: Vec2, puddle: &Puddle, side: f32) {
    let mut lobes = [(Vec2::ZERO, Vec2::ZERO); LOBES];
    for (slot, lobe) in lobes.iter_mut().zip(puddle.ellipses()) {
        *slot = lobe;
    }
    if lobes
        .iter()
        .any(|(_, radii)| !(radii.x > 0.0 && radii.y > 0.0))
    {
        return;
    }
    let [edge, body, gloss] = match puddle.liquid {
        Liquid::Mud => MUD_TONES,
        Liquid::Water => WATER_TONES,
    };
    let core = lobes[0].1;
    let shine = (
        puddle.center + Vec2::new(core.x * GLOSS_AT.x, core.y * GLOSS_AT.y),
        core * GLOSS_SIZE,
    );
    let Some(bounds) = lobes
        .iter()
        .map(|&(center, radii)| Rect::new(center - radii, center + radii))
        .reduce(Rect::union)
    else {
        return;
    };
    let span = |lo: f32, hi: f32| (lo / side).floor() as i32..=(hi / side).ceil() as i32;
    for j in span(bounds.min.y, bounds.max.y) {
        for i in span(bounds.min.x, bounds.max.x) {
            let corner = Vec2::new(i as f32, j as f32) * side;
            let middle = corner + Vec2::new(side, side) * 0.5;
            let deep = lobes
                .iter()
                .map(|&lobe| depth(middle, lobe))
                .fold(f32::INFINITY, f32::min);
            let even = (i + j) % 2 == 0;
            let tone = match deep {
                d if d >= 1.0 => None,
                d if d >= FRINGE => even.then_some(edge),
                d if d >= RIM => Some(edge),
                d if d >= SHORE => Some(if even { edge } else { body }),
                _ => match depth(middle, shine) {
                    g if g < GLOSS_CORE => Some(gloss),
                    g if g < 1.0 && even => Some(gloss),
                    _ => Some(body),
                },
            };
            if let Some(color) = tone {
                let min = corner - origin;
                canvas.fill_round_rect(
                    Rect::new(min, min + Vec2::new(side, side)),
                    0.0,
                    color,
                    1.0,
                );
            }
        }
    }
}

/// Насколько точка глубоко в эллипсе: 0 — середина, 1 — край.
fn depth(point: Vec2, (center, radii): (Vec2, Vec2)) -> f32 {
    let d = point - center;
    ((d.x / radii.x).powi(2) + (d.y / radii.y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    const SIDE: usize = 96;

    fn drawn(origin: Vec2) -> Vec<u32> {
        let mut mud = Mud::new(1);
        mud.pour(Vec2::new(48.0, 48.0), 30.0, 60.0, &mut Rng::new(3));
        mud.update(1.0);
        let mut pixels = vec![0u32; SIDE * SIDE];
        let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
        draw(&mut canvas, origin, &mud, 2);
        pixels
    }

    #[test]
    fn puddle_is_whole_opaque_cells_of_its_three_tones() {
        let pixels = drawn(Vec2::ZERO);
        let count = |color: Color| {
            pixels
                .iter()
                .filter(|p| **p == 0xFF00_0000 | color.0)
                .count()
        };
        let [edge, body, gloss] = MUD_TONES;
        assert!(count(body) > 100 && count(edge) > 20 && count(gloss) > 0);
        assert!(pixels.iter().all(|p| *p == 0 || *p >> 24 == 0xFF));
        for y in (0..SIDE).step_by(2) {
            for x in (0..SIDE).step_by(2) {
                let at = |dx: usize, dy: usize| pixels[(y + dy) * SIDE + x + dx];
                assert!(
                    [at(1, 0), at(0, 1), at(1, 1)]
                        .iter()
                        .all(|p| *p == at(0, 0))
                );
            }
        }
    }

    #[test]
    fn edge_dissolves_in_a_checkerboard() {
        let pixels = drawn(Vec2::ZERO);
        let edge = 0xFF00_0000 | MUD_TONES[0].0;
        let at = |x: usize, y: usize| pixels[y * SIDE + x];
        let holes = (2..SIDE - 2)
            .step_by(2)
            .flat_map(|y| (2..SIDE - 2).step_by(2).map(move |x| (x, y)))
            .filter(|&(x, y)| at(x, y) == 0 && at(x - 2, y) == edge && at(x + 2, y) == edge)
            .count();
        assert!(holes > 4, "{holes}");
    }
}
