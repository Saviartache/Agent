//! Ухо-лопух: плоский эллипс цветом чуть темнее тела; розовая изнанка — только когда
//! ухо повёрнуто ею к зрителю.

use super::EAR;
use crate::math::Vec2;
use crate::render::creature::{Pen, toward_viewer};
use crate::render::{Canvas, Color};
use crate::rig::{Ear, Vec3};

pub(super) const INNER: Color = Color(0xE0_A4_B0);
/// Изнанка: доля полуосей и сдвиг к шарниру в долях полуоси наружу.
const INNER_SIZE: f32 = 0.68;
const INNER_SHIFT: f32 = 0.12;
/// С ребра изнанку не видно.
const INNER_FACING: f32 = 0.35;
/// Лопух чуть выгнут: с ребра он лист, а не черта.
const CUP: f32 = 0.35;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen, ear: &Ear) {
    let offset = ear.center - pen.pose.head.center;
    fill(canvas, pen, offset, ear.axes, EAR);
    if toward_viewer(ear.inner) < INNER_FACING {
        return;
    }
    let inner = offset - ear.axes[0] * INNER_SHIFT;
    fill(canvas, pen, inner, ear.axes.map(|a| a * INNER_SIZE), INNER);
}

fn fill(canvas: &mut Canvas, pen: &Pen, offset: Vec3, axes: [Vec3; 2], color: Color) {
    let oval = pen.disc_on_head(offset, axes);
    let radii = Vec2::new(oval.radii.x, oval.radii.y.max(oval.radii.x * CUP));
    canvas.fill_ellipse(oval.center, radii, oval.rotation, color, 1.0);
}
