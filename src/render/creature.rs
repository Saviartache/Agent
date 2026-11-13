//! Отрисовка процедурного тела из позы: общее для всех видов — холст, тень,
//! проекция и мозаика; сам вид рисуют `goose`, `pig`, `elephant`, `raccoon` и `penguin`.

use super::scene::PixelRect;
use super::shadow::{self, Oval};
use super::{Canvas, Color, elephant, goose, penguin, pig, pixel, raccoon};
use crate::math::Vec2;
use crate::rig::{Frame, Pose, RigKind, Vec3, flatten};

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
        RigKind::Elephant => elephant::draw(canvas, &pen),
        RigKind::Raccoon => raccoon::draw(canvas, &pen),
        RigKind::Penguin => penguin::draw(canvas, &pen),
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

/// Смещение от центра головы на экране: проекция камеры, а не сжатие тела.
fn head_flat(offset: Vec3) -> Vec2 {
    Vec2::new(offset.x, offset.y * VIEW_SIN - offset.z * VIEW_COS)
}

pub(super) struct Pen<'a> {
    pub origin: Vec2,
    pub pose: &'a Pose,
}

/// Нога четвероногого: от бедра (плеча) к ступне.
pub(super) struct Leg {
    /// Верх: бедро в теле у дальней ноги, место выхода из брюха у ближней.
    pub top: Vec3,
    pub sole: Vec3,
    /// Насколько бедро ближе центра тела к зрителю, px.
    depth: f32,
    pub layer: LegLayer,
    /// Масштаб толщины: поджатая нога тает под телом.
    pub size: f32,
}

/// Когда нога рисуется: до тела, между телом и головой, после головы.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LegLayer {
    Back,
    Middle,
    Front,
}

/// Силуэт эллипсоида или плоского эллипса по полуосям на экране.
fn silhouette<const N: usize>(center: Vec2, axes: [Vec2; N]) -> Oval {
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for a in axes {
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
        self.at(self.pose.head.center) + head_flat(offset)
    }

    /// Эллипс или эллипсоид у головы: центр от её центра и полуоси, той же проекцией.
    pub fn disc_on_head<const N: usize>(&self, offset: Vec3, axes: [Vec3; N]) -> Oval {
        silhouette(self.on_head(offset), axes.map(head_flat))
    }

    pub fn body(&self) -> Oval {
        let body = &self.pose.body;
        self.ellipsoid(body.center, &body.frame(), body.radii)
    }

    /// Силуэт эллипсоида с полуосями `radii` вдоль осей `frame`.
    pub fn ellipsoid(&self, center: Vec3, frame: &Frame, radii: [f32; 3]) -> Oval {
        let axes = [
            frame.forward * radii[0],
            frame.right * radii[1],
            frame.up * radii[2],
        ];
        silhouette(self.at(center), axes.map(flatten))
    }

    /// Четыре ноги от дальней к ближней. Бедро на дальней от зрителя стороне тела —
    /// нога за телом и видна только ниже него; на ближней — поверх тела от брюха и, если
    /// голова ближе, под ней. `hip` — бедро в долях полуосей тела (по ходу, вбок, вниз),
    /// `max` — длина ноги, design px.
    pub fn legs(&self, hip: [f32; 3], max: f32) -> [Leg; 4] {
        let (pose, body) = (self.pose, &self.pose.body);
        let out = 1.0 - pose.tuck;
        // Поджатая наполовину нога короче, но не тоньше
        let size = pose.scale * (2.0 * out).min(1.0);
        let frame = body.frame();
        let forward = frame.forward * (body.radii[0] * hip[0]);
        let right = frame.right * (body.radii[1] * hip[1]);
        let down = frame.up * (-body.radii[2] * hip[2]);
        let fore = pose.fore.unwrap_or(pose.feet);
        let head = toward_viewer(pose.head.center - body.center);
        let feet = [
            (pose.feet[0], -1.0, -1.0),
            (pose.feet[1], -1.0, 1.0),
            (fore[0], 1.0, -1.0),
            (fore[1], 1.0, 1.0),
        ];
        let mut legs = feet.map(|(foot, along, side)| {
            let hip = body.center + forward * along + right * side + down;
            let reach = (foot - hip).length().max(f32::EPSILON);
            // Отставшая на бегу нога не вытягивается: ступня отрывается от земли
            let sole = hip.lerp(foot, out * (max * pose.scale / reach).min(1.0));
            let depth = toward_viewer(hip - body.center);
            let exit = self.body_exit(hip, sole);
            // Ступня в теле — ноги не видно, тело её закрывает
            let (top, layer) = if depth < 0.0 || exit >= 1.0 {
                (hip, LegLayer::Back)
            } else if depth < head {
                (hip.lerp(sole, exit), LegLayer::Middle)
            } else {
                (hip.lerp(sole, exit), LegLayer::Front)
            };
            Leg {
                top,
                sole,
                depth,
                layer,
                size,
            }
        });
        legs.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        legs
    }

    /// Смещение от центра тела в его осях, где полуоси — единицы.
    fn body_local(&self, v: Vec3) -> Vec3 {
        let body = &self.pose.body;
        let frame = body.frame();
        Vec3::new(
            v.dot(frame.forward) / body.radii[0],
            v.dot(frame.right) / body.radii[1],
            v.dot(frame.up) / body.radii[2],
        )
    }

    /// Доля пути от `from` к `to`, где он выходит из тела; 0 — `from` уже снаружи.
    fn body_exit(&self, from: Vec3, to: Vec3) -> f32 {
        let p = self.body_local(from - self.pose.body.center);
        let d = self.body_local(to - from);
        let (a, b, c) = (d.dot(d), p.dot(d), p.dot(p) - 1.0);
        if c >= 0.0 || a <= f32::EPSILON {
            return 0.0;
        }
        (-b + (b * b - a * c).sqrt()) / a
    }

    /// Точку закрывает тело: луч от неё к зрителю входит в эллипсоид.
    pub fn behind_body(&self, at: Vec3) -> bool {
        let p = self.body_local(at - self.pose.body.center);
        let d = self.body_local(TOWARD_VIEWER);
        // |p + t·d| = 1: дальний корень впереди точки
        let (a, b, c) = (d.dot(d), p.dot(d), p.dot(p) - 1.0);
        let disc = b * b - a * c;
        disc >= 0.0 && disc.sqrt() >= b
    }

    /// Точка тела на ближней к зрителю стороне от его центра: хвост поверх тела.
    pub fn near_side(&self, at: Vec3) -> bool {
        faces_viewer(at - self.pose.body.center)
    }

    /// Пятно под телом; в прыжке и в руках оно меньше и бледнее.
    fn shadow(&self, canvas: &mut Canvas) {
        let (pose, body) = (self.pose, &self.pose.body);
        let k = (0.55 + 0.45 * pose.shadow) * SHADOW_SPREAD;
        let forward = Vec3::on_ground(body.forward * (body.radii[0] * k), 0.0);
        let right = Vec3::new(-body.forward.y, body.forward.x, 0.0) * (body.footprint_width() * k);
        let ground = Vec3::on_ground(body.center.ground(), 0.0);
        let oval = silhouette(pose.screen(ground), [forward, right].map(flatten));
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

    /// Четвероногое в профиль, у которого видны ноги только ближней (`near`) или только
    /// дальней стороны: ступни другой спрятаны в тело.
    pub fn one_side_legs(kind: RigKind, near: bool) -> Pose {
        let mut pose = pose(kind, Vec2::new(1.0, 0.0));
        let near_side = usize::from(toward_viewer(pose.body.frame().right) > 0.0);
        let hidden = if near { 1 - near_side } else { near_side };
        let inside = pose.body.center;
        pose.feet[hidden] = inside;
        if let Some(fore) = &mut pose.fore {
            fore[hidden] = inside;
        }
        pose
    }

    /// Пиксели ног и сколько из них лежат поверх тела — внутри его силуэта, без края.
    pub fn legs_over_body(pose: &Pose, pixels: &[u32], w: usize, leg: Color) -> (usize, usize) {
        let rect = rect(pose, Creature::extent(SCALE));
        let origin = Vec2::new(rect.x as f32, rect.y as f32) - Vec2::new(1.0, 1.0) * MARGIN as f32;
        let mut body = Pen { origin, pose }.body();
        body.radii = body.radii * 0.95;
        // Мозаика не оставляет тонкую ногу чистой: цвет близкий, а не точный
        let channels = |c: u32| [c >> 16 & 0xFF, c >> 8 & 0xFF, c & 0xFF];
        let close = |p: u32| {
            let (got, want) = (channels(p), channels(leg.0));
            p >> 24 == 0xFF && (0..3).all(|i| got[i].abs_diff(want[i]) <= 8)
        };
        let legs = pixels.iter().enumerate().filter(|(_, p)| close(**p));
        legs.fold((0, 0), |(all, over), (i, _)| {
            let at = Vec2::new((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            (all + 1, over + usize::from(body.contains(at)))
        })
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
