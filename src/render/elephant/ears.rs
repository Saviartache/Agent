//! Ухо-лопух из двух долей: от шарнира у затылка оно расширяется и закругляется, нижняя
//! доля длиннее. Ухо плоское: наружная сторона чуть темнее тела, изнанка — розовая в серой
//! кайме той же плоскости; к ребру они сужаются вместе, а с ребра ухо — серая лопатка.

use super::EAR;
use crate::math::Vec2;
use crate::render::creature::{Pen, toward_viewer};
use crate::render::shadow::Oval;
use crate::render::{Canvas, Color};
use crate::rig::Ear;

pub(super) const INNER: Color = Color(0xE0_A4_B0);
/// Доли в долях полуосей уха от шарнира: центр наружу и вниз, полуоси.
const LOBES: [[f32; 4]; 2] = [[0.7, 0.3, 0.75, 0.42], [0.85, 0.75, 0.75, 0.65]];
/// С ребра ухо не тоньше этого, design px: толща лопуха.
const EDGE: f32 = 1.8;
/// Кайма вокруг изнанки, design px.
const RIM: f32 = 1.4;
/// Изнанка сдвинута к шарниру на долю каймы: снаружи кайма шире.
const RIM_SHIFT: f32 = 1.0;
/// Изнанка видна спереди: проступает, пока нормаль поворачивается к зрителю от и до.
const INNER_FACING: [f32; 2] = [0.15, 0.3];

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen, ear: &Ear) {
    let lobes = lobes(pen, ear);
    for [oval, _] in &lobes {
        canvas.fill_ellipse(oval.center, oval.radii, oval.rotation, EAR, 1.0);
    }
    for [_, inner] in &lobes {
        canvas.fill_ellipse(inner.center, inner.radii, inner.rotation, INNER, 1.0);
    }
}

/// Доли: серый силуэт и изнанка в нём (нулевая — не видна). Изнанка — доля без каймы
/// `RIM`; пока доля сужается до лопатки, изнанка целиком сжимается в точку.
fn lobes(pen: &Pen, ear: &Ear) -> [[Oval; 2]; 2] {
    let (s, rim) = (pen.pose.scale, RIM * pen.pose.scale);
    let facing = ramp(toward_viewer(ear.inner), INNER_FACING);
    let hinge = pen.on_head(ear.root - pen.pose.head.center);
    LOBES.map(|shape| {
        let oval = lobe(pen, ear, shape);
        let k = facing * ramp(oval.radii.y, [EDGE * s, EDGE * s + rim]);
        let inward = (hinge - oval.center).normalized() * (rim * RIM_SHIFT * k);
        let inner = Oval {
            center: oval.center + inward,
            radii: Vec2::new(oval.radii.x - rim, oval.radii.y - rim) * k,
            rotation: oval.rotation,
        };
        [oval, inner]
    })
}

/// 0 до `from`, 1 после `to`, между — прямой.
fn ramp(x: f32, [from, to]: [f32; 2]) -> f32 {
    ((x - from) / (to - from)).clamp(0.0, 1.0)
}

/// Доля изнанки в площади уха.
#[cfg(test)]
pub(super) fn inner_share(pen: &Pen, ear: &Ear) -> f32 {
    let (inner, all) = lobes(pen, ear)
        .iter()
        .fold((0.0, 0.0), |(inner, all), [oval, pink]| {
            (
                inner + pink.radii.x * pink.radii.y,
                all + oval.radii.x * oval.radii.y,
            )
        });
    inner / all
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
        .map(|shape| height(&lobe(pen, ear, shape)))
        .fold(f32::INFINITY, f32::min)
}

/// Доля в плоскости уха; с ребра — лопатка толщиной `EDGE`.
fn lobe(pen: &Pen, ear: &Ear, [out, down, a, b]: [f32; 4]) -> Oval {
    let [blade, hang] = ear.axes;
    let center = ear.root + blade * out + hang * down - pen.pose.head.center;
    let mut oval = pen.disc_on_head(center, [blade * a, hang * b]);
    oval.radii.y = oval.radii.y.max(EDGE * pen.pose.scale);
    oval
}
