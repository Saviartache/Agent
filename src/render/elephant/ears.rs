//! Ухо-лопух из двух долей: от шарнира у затылка оно расширяется и закругляется, нижняя
//! доля длиннее. Наружная сторона чуть темнее тела; повёрнутое к зрителю изнанкой —
//! розовое в серой кайме.

use super::EAR;
use crate::math::Vec2;
use crate::render::creature::{Pen, toward_viewer};
use crate::render::shadow::Oval;
use crate::render::{Canvas, Color};
use crate::rig::{Ear, Vec3};

pub(super) const INNER: Color = Color(0xE0_A4_B0);
/// Доли в долях полуосей уха от шарнира: центр наружу и вниз, полуоси.
const LOBES: [[f32; 4]; 2] = [[0.7, 0.3, 0.75, 0.42], [0.85, 0.75, 0.75, 0.65]];
/// Толща вдоль хода головы в долях полуоси наружу: с ребра ухо не тоньше её.
const THICK: f32 = 0.9;
/// Кайма вокруг изнанки, design px.
const RIM: f32 = 1.2;
/// Почти с ребра в чашу не видно.
const INNER_FACING: f32 = 0.3;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen, ear: &Ear) {
    let lobes = LOBES.map(|shape| lobe(pen, ear, shape));
    for [face, bulk] in &lobes {
        for oval in [face, bulk] {
            canvas.fill_ellipse(oval.center, oval.radii, oval.rotation, EAR, 1.0);
        }
    }
    if toward_viewer(ear.inner) < INNER_FACING {
        return;
    }
    let rim = RIM * pen.pose.scale;
    for [oval, _] in &lobes {
        let radii = Vec2::new(oval.radii.x - rim, oval.radii.y - rim);
        // Тоньше каймы — не лоскут, а щель: не рисуется
        if radii.y >= rim {
            canvas.fill_ellipse(oval.center, radii, oval.rotation, INNER, 1.0);
        }
    }
}

/// Верх уха на холсте.
#[cfg(test)]
pub(super) fn top(pen: &Pen, ear: &Ear) -> f32 {
    let height = |o: &Oval| {
        let (sin, cos) = o.rotation.sin_cos();
        o.center.y - (o.radii.x * sin).hypot(o.radii.y * cos)
    };
    LOBES
        .into_iter()
        .flat_map(|shape| lobe(pen, ear, shape))
        .map(|o| height(&o))
        .fold(f32::INFINITY, f32::min)
}

/// Доля: кромка с изнанкой и толща лопуха вдоль хода головы — анфас она за изнанкой,
/// сбоку и с ребра даёт ширину.
fn lobe(pen: &Pen, ear: &Ear, [out, down, a, b]: [f32; 4]) -> [Oval; 2] {
    let [blade, hang] = ear.axes;
    let center = ear.root + blade * out + hang * down - pen.pose.head.center;
    let axes = [blade * a, hang * b];
    let ahead = pen.pose.head_frame().forward;
    let thick = Vec3::new(ahead.x, ahead.y, 0.0).normalized() * (blade.length() * a * THICK);
    [
        pen.disc_on_head(center, axes),
        pen.disc_on_head(center, [axes[0], axes[1], thick]),
    ]
}
