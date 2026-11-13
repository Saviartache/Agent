//! Отрисовка процедурного тела из позы: общее для всех видов — холст, тень,
//! проекция и мозаика; сам вид рисуют `goose`, `pig` и `dachshund`.

use super::scene::PixelRect;
use super::shadow::{self, Oval};
use super::{Canvas, Color, dachshund, goose, pig, pixel};
//! проекция и мозаика; сам вид рисуют `goose`, `pig` и `elephant`.

use super::scene::PixelRect;
use super::shadow::{self, Oval};
use super::{Canvas, Color, elephant, goose, pig, pixel};
use crate::math::Vec2;
use crate::rig::{Pose, RigKind, Vec3, flatten};

pub(super) const WHITE: Color = Color(0xFF_FF_FF);
/// Нужна только затем, чтобы белое не пропадало на белом окне.
pub(super) const OUTLINE: Color = Color(0xB9_C0_CB);
pub(super) const INK: Color = Color(0x1B_1F_29);
/// design px, умножается на `pose.scale`.
pub(super) const OUTLINE_WIDTH: f32 = 0.7;
/// Направление на камеру в `(x, y, z)`: она смотрит на голову сверху под 30°.
const TOWARD_VIEWER: Vec3 = Vec3::new(0.0, VIEW_COS, VIEW_SIN);
const VIEW_SIN: f32 = 0.5;
const VIEW_COS: f32 = 0.866;
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

/// `origin` — мировая точка левого верхнего угла холста.
pub fn draw(canvas: &mut Canvas, origin: Vec2, pose: &Pose) {
    paint(canvas, origin, pose);
    pixel::pixelate(canvas, origin, pose.scale);
}

/// Как `draw`, но без мозаики: её кладёт тот, кто рисует холст целиком.
pub fn paint(canvas: &mut Canvas, origin: Vec2, pose: &Pose) {
    if pose.scale <= 0.0 {
        return;
    }
    let pen = Pen { origin, pose };
    pen.shadow(canvas);
    match pose.kind {
        RigKind::Goose => goose::draw(canvas, &pen),
        RigKind::Pig => pig::draw(canvas, &pen),
        RigKind::Dachshund => dachshund::draw(canvas, &pen),
        RigKind::Elephant => elephant::draw(canvas, &pen),
    }
}

/// Насколько точка, отложенная от центра шара на `offset`, ближе к зрителю, чем центр.
pub(super) fn toward_viewer(offset: Vec3) -> f32 {
    offset.dot(TOWARD_VIEWER)
}

pub(super) fn faces_viewer(offset: Vec3) -> bool {
    toward_viewer(offset) >= 0.0
}

/// Голова отвернулась от зрителя настолько, что глаз не видно ни одного.
pub(super) fn turned_away(forward: Vec3) -> bool {
    toward_viewer(forward) < -0.25
}

/// Нутро рта и ноздри.
pub(super) const MOUTH: Color = Color(0x8E_3E_4A);
pub(super) const TONGUE: Color = Color(0xE8_5F_7A);

/// Глаз-крестик: две короткие капсулы крест-накрест; `radius` — полуразмер.
pub(super) fn cross(canvas: &mut Canvas, center: Vec2, radius: f32, width: f32) {
    let d = Vec2::new(radius, radius);
    canvas.fill_capsule(center - d, center + d, width, width, INK, 1.0);
    let e = Vec2::new(radius, -radius);
    canvas.fill_capsule(center - e, center + e, width, width, INK, 1.0);
}

pub(super) struct Pen<'a> {
    pub origin: Vec2,
    pub pose: &'a Pose,
}

/// Силуэт эллипсоида с осями `axes`.
fn silhouette(center: Vec2, axes: [Vec3; 3]) -> Oval {
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for a in axes.map(flatten) {
        xx += a.x * a.x;
        yy += a.y * a.y;
        xy += a.x * a.y;
    }
    let mean = (xx + yy) * 0.5;
    let diff = (xx - yy) * 0.5;
    let root = diff.hypot(xy);
    Oval {
        center,
        radii: Vec2::new((mean + root).sqrt(), (mean - root).max(0.0).sqrt()),
        rotation: 0.5 * xy.atan2(diff),
    }
}

impl Pen<'_> {
    pub fn at(&self, point: Vec3) -> Vec2 {
        self.pose.screen(point) - self.origin
    }

    /// Точка на голове, отложенная от её центра на `offset`. Голова рисуется
    /// кругом, поэтому точка кладётся проекцией шара, а не сжатием тела.
    pub fn on_head(&self, offset: Vec3) -> Vec2 {
        let flat = Vec2::new(offset.x, offset.y * VIEW_SIN - offset.z * VIEW_COS);
        self.at(self.pose.head.center) + flat
    }

    pub fn body(&self) -> Oval {
        let body = &self.pose.body;
        let frame = body.frame();
        let axes = [
            frame.forward * body.radii[0],
            frame.right * body.radii[1],
            frame.up * body.radii[2],
        ];
        silhouette(self.at(body.center), axes)
    }

    /// Пятно под телом; в прыжке и в руках оно меньше и бледнее.
    fn shadow(&self, canvas: &mut Canvas) {
        let (pose, body) = (self.pose, &self.pose.body);
        let k = (0.55 + 0.45 * pose.shadow) * SHADOW_SPREAD;
        let forward = Vec3::on_ground(body.forward * (body.radii[0] * k), 0.0);
        let right = Vec3::new(-body.forward.y, body.forward.x, 0.0) * (body.footprint_width() * k);
        let ground = Vec3::on_ground(body.center.ground(), 0.0);
        let oval = silhouette(pose.screen(ground), [forward, right, Vec3::ZERO]);
        let (cell, opacity) = (pixel::cell(pose.scale), shadow::OPACITY * pose.shadow);
        shadow::dotted(canvas, self.origin, &oval, cell, opacity);
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;
    use crate::rig::Creature;

    pub const SCALE: f32 = 1.5;
    pub const AT: Vec2 = Vec2::new(300.5, 400.25);
    /// Запас вокруг холста, чтобы видеть утечки за его край.
    pub const MARGIN: usize = 40;
    pub const HEADINGS: [Vec2; 4] = [
        Vec2::new(1.0, 0.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(0.0, -1.0),
    ];

    pub fn pose(kind: RigKind, heading: Vec2) -> Pose {
        Creature::new(kind, AT, heading, SCALE).pose().clone()
    }

    /// Рисует в буфер, больший холста на `MARGIN` со всех сторон.
    pub fn render(pose: &Pose) -> (Vec<u32>, usize, usize) {
        let rect = rect(pose, Creature::extent(SCALE));
        let (w, h) = (
            rect.width as usize + 2 * MARGIN,
            rect.height as usize + 2 * MARGIN,
        );
        let mut pixels = vec![0u32; w * h];
        let mut canvas = Canvas::new(&mut pixels, w, w as u32, h as u32).unwrap();
        let origin = Vec2::new(rect.x as f32 - MARGIN as f32, rect.y as f32 - MARGIN as f32);
        draw(&mut canvas, origin, pose);
        (pixels, w, h)
    }

    pub fn opaque(pixels: &[u32]) -> usize {
        pixels.iter().filter(|p| **p >> 24 != 0).count()
    }

    pub fn solid(pixels: &[u32], color: Color) -> usize {
        pixels
            .iter()
            .filter(|p| **p == 0xFF00_0000 | color.0)
            .count()
    }

    pub fn outside_rect(pixels: &[u32], w: usize, h: usize) -> usize {
        let inner = MARGIN..w - MARGIN;
        let rows = MARGIN..h - MARGIN;
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|(x, y)| !(inner.contains(x) && rows.contains(y)))
            .filter(|(x, y)| pixels[y * w + x] >> 24 != 0)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::rig::Creature;

    #[test]
    fn rect_is_a_square_around_the_ground_point() {
        let pose = pose(RigKind::Goose, Vec2::new(1.0, 0.0));
        let extent = Creature::extent(SCALE);
        let rect = rect(&pose, extent);
        assert_eq!(rect.width, rect.height);
        assert_eq!(rect.width, (extent * 2.0).ceil() as u32);
        assert_eq!(rect.x, (AT.x - extent).round() as i32);
        assert_eq!(rect.y, (AT.y - extent).round() as i32);
    }

    #[test]
    fn high_flight_stays_on_the_canvas() {
        for kind in RigKind::ALL {
            let mut rig = Creature::new(kind, AT, Vec2::new(1.0, 0.0), SCALE);
            let input = crate::rig::RigInput {
                pos: AT,
                vel: Vec2::ZERO,
                heading: Vec2::new(1.0, 0.0),
                look_at: None,
                gesture: crate::rig::Gesture::Idle,
                progress: 0.0,
                scale: SCALE,
                grounded: true,
                antic: crate::rig::Antic {
                    lift: 60.0,
                    wings: 1.0,
                    ..Default::default()
                },
            };
            let (mut rng, mut steps) = (crate::rng::Rng::new(7), Vec::new());
            for _ in 0..120 {
                rig.update(1.0 / 60.0, &input, &mut rng, &mut steps);
            }
            let (pixels, w, h) = render(rig.pose());
            assert_eq!(outside_rect(&pixels, w, h), 0, "{kind:?}");
        }
    }

    #[test]
    fn shadow_is_dotted_and_fades_when_lifted() {
        for kind in RigKind::ALL {
            let grounded = pose(kind, Vec2::new(1.0, 0.0));
            let mut lifted = grounded.clone();
            lifted.shadow = 0.0;
            let (with, w, _) = render(&grounded);
            let (without, ..) = render(&lifted);
            assert!(opaque(&with) > opaque(&without));
            // Под тенью остаются прозрачные пиксели: строка у земли не сплошная
            let row = (AT.y - (AT.y - Creature::extent(SCALE)).round()) as usize + MARGIN + 4;
            let line = &with[row * w..(row + 1) * w];
            let (first, last) = (
                line.iter().position(|p| *p >> 24 != 0).unwrap(),
                line.iter().rposition(|p| *p >> 24 != 0).unwrap(),
            );
            assert!(line[first..last].iter().any(|p| *p >> 24 == 0), "{kind:?}");
        }
    }

    #[test]
    fn posed_bodies_draw_inside_the_rect() {
        use crate::rig::{Antic, Gesture, RigInput};
        let squash = Vec2::new(1.6, 0.25);
        let antics = [
            Antic {
                roll: std::f32::consts::PI,
                ..Antic::default()
            },
            Antic {
                roll: 1.2,
                ..Antic::default()
            },
            Antic {
                squash,
                ..Antic::default()
            },
            Antic {
                lift: 25.0,
                ..Antic::default()
            },
        ];
        for kind in RigKind::ALL {
            for heading in HEADINGS {
                for antic in antics {
                    let mut body = Creature::new(kind, AT, heading, SCALE);
                    let input = RigInput {
                        pos: AT,
                        vel: Vec2::ZERO,
                        heading,
                        look_at: None,
                        gesture: Gesture::Idle,
                        progress: 0.5,
                        scale: SCALE,
                        grounded: true,
                        antic,
                    };
                    for _ in 0..60 {
                        body.update(
                            1.0 / 60.0,
                            &input,
                            &mut crate::rng::Rng::new(7),
                            &mut Vec::new(),
                        );
                    }
                    let (pixels, w, h) = render(body.pose());
                    assert!(opaque(&pixels) > 300, "{kind:?} {heading:?} {antic:?}");
                    assert_eq!(
                        outside_rect(&pixels, w, h),
                        0,
                        "{kind:?} {heading:?} {antic:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn degenerate_and_broken_poses_draw_nothing_or_stay_quiet() {
        for kind in RigKind::ALL {
            let mut gone = pose(kind, Vec2::new(1.0, 0.0));
            gone.zoomed(0.0);
            let (pixels, ..) = render(&gone);
            assert_eq!(opaque(&pixels), 0);
            let mut broken = pose(kind, Vec2::new(1.0, 0.0));
            broken.body.center.x = f32::NAN;
            broken.head.center.z = f32::INFINITY;
            render(&broken);
        }
    }
}
