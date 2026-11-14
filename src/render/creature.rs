//! Общее для тел всех видов, что не зависит от скелета: холст вокруг точки на земле
//! и тень под телом. Само тело рисует `Puppet`.

use super::scene::PixelRect;
use super::shadow::{self, Oval};
use super::{Canvas, pixel};
use crate::math::Vec2;
use crate::rig::{Pose, Vec3, flatten};

/// Тень чуть шире тела.
const SHADOW_SPREAD: f32 = 1.1;

/// Холст вокруг точки на земле: квадрат со стороной `2 × extent`; в полёте поднимается
/// за телом, тень на земле остаётся в нижней половине.
pub fn rect(pose: &Pose, extent: f32) -> PixelRect {
    let rise = pose.rise().min(extent * 0.5);
    PixelRect {
        x: (pose.pos.x - extent).round() as i32,
        y: (pose.pos.y - extent - rise).round() as i32,
        width: (extent * 2.0).ceil().max(1.0) as u32,
        height: (extent * 2.0).ceil().max(1.0) as u32,
    }
}

/// Эллипс на земле по двум полуосям на экране.
fn oval(center: Vec2, axes: [Vec2; 2]) -> Oval {
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for a in axes {
        xx += a.x * a.x;
        yy += a.y * a.y;
        xy += a.x * a.y;
    }
    let (mean, diff) = ((xx + yy) * 0.5, (xx - yy) * 0.5);
    let root = diff.hypot(xy);
    Oval {
        center,
        radii: Vec2::new((mean + root).sqrt(), (mean - root).max(0.0).sqrt()),
        rotation: 0.5 * xy.atan2(diff),
    }
}

/// Пятно под телом; в прыжке и в руках оно меньше и бледнее. `origin` — мировая точка
/// левого верхнего угла холста.
pub(super) fn shadow(canvas: &mut Canvas, origin: Vec2, pose: &Pose) {
    if pose.scale.is_nan() || pose.scale <= 0.0 {
        return;
    }
    let body = &pose.body;
    let k = (0.55 + 0.45 * pose.shadow) * SHADOW_SPREAD;
    let forward = Vec3::on_ground(body.forward * (body.radii[0] * k), 0.0);
    let right = Vec3::new(-body.forward.y, body.forward.x, 0.0) * (body.footprint_width() * k);
    let ground = Vec3::on_ground(body.center.ground(), 0.0);
    let spot = oval(pose.screen(ground), [forward, right].map(flatten));
    let (cell, opacity) = (pixel::cell(pose.scale), shadow::OPACITY * pose.shadow);
    shadow::dotted(canvas, origin, &spot, cell, opacity);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::{Creature, RigKind};

    const SCALE: f32 = 1.5;
    const AT: Vec2 = Vec2::new(300.5, 400.25);

    fn pose(kind: RigKind) -> Pose {
        Creature::new(kind, AT, Vec2::new(1.0, 0.0), SCALE)
            .pose()
            .clone()
    }

    fn shade(pose: &Pose) -> (Vec<u32>, usize) {
        let rect = rect(pose, Creature::extent(SCALE));
        let (w, h) = (rect.width as usize, rect.height as usize);
        let mut pixels = vec![0u32; w * h];
        let mut canvas = Canvas::new(&mut pixels, w, w as u32, h as u32).unwrap();
        shadow(&mut canvas, Vec2::new(rect.x as f32, rect.y as f32), pose);
        (pixels, w)
    }

    #[test]
    fn rect_is_a_square_around_the_ground_point() {
        let pose = pose(RigKind::Goose);
        let extent = Creature::extent(SCALE);
        let rect = rect(&pose, extent);
        assert_eq!(rect.width, rect.height);
        assert_eq!(rect.width, (extent * 2.0).ceil() as u32);
        assert_eq!(rect.x, (AT.x - extent).round() as i32);
        assert_eq!(rect.y, (AT.y - extent).round() as i32);
    }

    #[test]
    fn shadow_is_dotted_and_fades_when_lifted() {
        for kind in RigKind::ALL {
            let grounded = pose(kind);
            let mut lifted = grounded.clone();
            lifted.shadow = 0.0;
            let opaque = |pixels: &[u32]| pixels.iter().filter(|p| **p >> 24 != 0).count();
            let ((with, w), (without, _)) = (shade(&grounded), shade(&lifted));
            assert!(opaque(&with) > 40 && opaque(&without) == 0, "{kind:?}");
            // Сквозь пятно виден рабочий стол: строка у земли не сплошная
            let row = (AT.y - (AT.y - Creature::extent(SCALE)).round()) as usize + 4;
            let line = &with[row * w..(row + 1) * w];
            let (first, last) = (
                line.iter().position(|p| *p >> 24 != 0).unwrap(),
                line.iter().rposition(|p| *p >> 24 != 0).unwrap(),
            );
            assert!(line[first..last].iter().any(|p| *p >> 24 == 0), "{kind:?}");
        }
    }

    #[test]
    fn broken_pose_casts_no_shadow() {
        let mut broken = pose(RigKind::Pig);
        broken.scale = f32::NAN;
        assert!(shade(&broken).0.iter().all(|pixel| *pixel == 0));
        let mut wild = pose(RigKind::Pig);
        wild.body.center.x = f32::NAN;
        shade(&wild);
    }
}
