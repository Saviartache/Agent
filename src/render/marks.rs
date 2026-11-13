//! Частицы-знаки: точка, «Z», нотка, перо, сердечко, кольцо.

use std::f32::consts::TAU;

use super::{Canvas, Color};
use crate::effects::particles::{Particle, Shape};
use crate::math::Vec2;

const RING_SEGMENTS: usize = 24;
const WHITE_EDGE: Color = Color(0xB9_C0_CB);

pub(super) fn draw(canvas: &mut Canvas, origin: Vec2, particle: &Particle) {
    let (c, r) = (particle.pos - origin, particle.radius);
    let (color, o) = (Color(particle.color), particle.opacity);
    let line = (r * 0.34).max(1.3);
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    match particle.shape {
        Shape::Dot => canvas.fill_circle(c, r, color, o),
        Shape::Z => {
            canvas.fill_capsule(at(-0.7, -0.7), at(0.7, -0.7), line, line, color, o);
            canvas.fill_capsule(at(0.7, -0.7), at(-0.7, 0.7), line, line, color, o);
            canvas.fill_capsule(at(-0.7, 0.7), at(0.7, 0.7), line, line, color, o);
        }
        Shape::Note => {
            let (stem, flag) = (line * 0.75, line * 0.65);
            canvas.fill_ellipse(
                at(-0.35, 0.65),
                Vec2::new(r * 0.62, r * 0.46),
                -0.4,
                color,
                o,
            );
            canvas.fill_capsule(at(0.15, 0.55), at(0.15, -0.95), stem, stem, color, o);
            canvas.fill_capsule(at(0.15, -0.95), at(0.8, -0.45), flag, flag * 0.6, color, o);
        }
        Shape::Feather => {
            let (sin, cos) = particle.angle.sin_cos();
            let along = |t: f32| c + Vec2::new(cos * t * r, sin * t * r);
            for (grow, shade) in [(1.15, WHITE_EDGE), (1.0, color)] {
                let wide = Vec2::new(r, r * 0.38) * grow;
                canvas.fill_ellipse(c, wide, particle.angle, shade, o);
                let tail = r * 0.09 * grow;
                canvas.fill_capsule(along(-0.9), along(-1.4), tail, tail * 0.6, shade, o);
            }
        }
        Shape::Heart => {
            canvas.fill_circle(at(-0.42, -0.2), r * 0.55, color, o);
            canvas.fill_circle(at(0.42, -0.2), r * 0.55, color, o);
            canvas.fill_capsule(at(-0.7, 0.05), at(0.0, 0.85), r * 0.34, r * 0.1, color, o);
            canvas.fill_capsule(at(0.7, 0.05), at(0.0, 0.85), r * 0.34, r * 0.1, color, o);
        }
        Shape::Ring => {
            let thick = (r * 0.14).max(1.0);
            // Внешний край — ровно `radius`
            let inner = (r - thick).max(0.0);
            let point =
                |i: usize| c + Vec2::from_angle(i as f32 * TAU / RING_SEGMENTS as f32) * inner;
            for i in 0..RING_SEGMENTS {
                canvas.fill_capsule(point(i), point(i + 1), thick, thick, color, o);
            }
        }
    }
}
