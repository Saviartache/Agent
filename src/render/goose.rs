//! Гусь, намеренно скупой: тело, шея, голова, клюв и две лапки, не соединённые
//! с телом. Белое сливается в один силуэт с еле заметной обводкой.

use super::creature::{
    INK, MOUTH, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer, toward_viewer,
    turned_away,
};
use super::pig::MUD;
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

const ORANGE: Color = Color(0xFF_9A_1F);
/// Кожа проплешины.
const SKIN: Color = Color(0xF4_B6_B0);

/// Проплешина: радиус по `bald`, design px.
const BALD_RADIUS: [f32; 2] = [1.5, 3.5];
/// Центр проплешины в долях полуосей тела: назад, вбок, вверх.
const BALD_AT: [f32; 3] = [-0.15, 0.6, 0.4];
/// Раскрытый клюв: угол половинки при `mouth = 1`, рад; радиусы половинки.
const GAPE: f32 = 0.5;
const HALF_RADII: [f32; 2] = [1.8, 0.9];
/// Приоткрытый клюв, из которого виден язык.
const TONGUE_GAPE: f32 = 0.2;
/// Язык набок: вбок и вниз от клюва в радиусах головы.
const TONGUE_AT: [f32; 2] = [0.55, 0.45];
const TONGUE_RADII: [f32; 2] = [1.3, 1.0];
/// Грязь на клюве: от какой доли длины, и толщина.
const MUD_FROM: f32 = 0.45;
const MUD_WIDTH: f32 = 1.15;
const CROSS_REACH: f32 = 1.0;
const CROSS_WIDTH: f32 = 0.55;
const WIDE_EYE: [f32; 2] = [2.5, 1.2];
/// Крыло: корень в долях полуосей тела (назад, вбок, вверх), длина в продольных полуосях.
const WING_ROOT: [f32; 3] = [-0.1, 0.8, 0.45];
const WING_LENGTH: f32 = 1.3;
const WING_RADII: [f32; 2] = [4.6, 2.0];
/// Два пера веером: подъём над горизонталью (рад) раскрытого крыла и откат назад, доля длины.
const WING_FEATHERS: [(f32, f32, f32); 2] = [(0.0, 0.0, 1.0), (-0.55, 0.35, 0.8)];
/// Размах взмаха по подъёму, рад.
const FLAP_SWING: f32 = 0.7;
const WING_REST_LIFT: f32 = 0.5;
/// Крыло ближе к зрителю этого порога рисуется поверх тела.
const WING_NEAR: f32 = 0.2;
/// Петля узла на шее: радиус, design px; номер точки шеи.
const KNOT_RADIUS: f32 = 5.5;
const KNOT_AT: usize = NECK_POINTS / 2;
const KNOT_SEGMENTS: usize = 12;

// Размеры — design px, умножаются на `pose.scale`.
const NECK_RADIUS: f32 = 3.2;
const BEAK_RADII: [f32; 2] = [2.3, 1.1];
const FOOT_RADIUS: f32 = 2.9;
const EYE_RADIUS: f32 = 1.3;
/// Глаз на голове: вбок, вперёд и вверх в долях её радиуса.
/// Выше и позади клюва: глаз остаётся на белом.
const EYE_AT: [f32; 3] = [0.5, 0.15, 0.55];
/// Основание клюва от центра головы, в долях её радиуса.
const BEAK_BASE: f32 = 0.9;

/// Клюв за головой, только когда голова отвернулась, — по ней, а не по телу.
/// В профиль он поверх: глаз виден, значит, и клюв.
pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let goose = Goose(pen);
    let beak_front = !turned_away(pen.pose.head_frame().forward);
    goose.feet(canvas, false);
    goose.wings(canvas, false);
    goose.white(canvas, OUTLINE, OUTLINE_WIDTH * pen.pose.scale);
    if !beak_front {
        goose.beak(canvas);
    }
    goose.white(canvas, WHITE, 0.0);
    goose.wings(canvas, true);
    goose.feet(canvas, true);
    if beak_front {
        goose.beak(canvas);
    }
    goose.eyes(canvas);
}

struct Goose<'a>(&'a Pen<'a>);

impl Goose<'_> {
    /// Всё белое одним цветом: швов между телом, шеей и головой нет.
    fn white(&self, canvas: &mut Canvas, color: Color, grow: f32) {
        let (pen, pose, s) = (self.0, self.0.pose, self.0.pose.scale);
        let body = pen.body();
        let radii = body.radii + Vec2::new(grow, grow);
        canvas.fill_ellipse(body.center, radii, body.rotation, color, 1.0);
        if grow == 0.0 {
            self.bald(canvas);
        }
        let radius = NECK_RADIUS * s + grow;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, color, 1.0);
        }
        self.knot(canvas, color, radius);
        canvas.fill_circle(
            pen.at(pose.head.center),
            pose.head.radius + grow,
            color,
            1.0,
        );
    }

    /// Лапы по одну сторону от белого: `front` — когда брюхо смотрит к зрителю.
    fn feet(&self, canvas: &mut Canvas, front: bool) {
        let pose = self.0.pose;
        let belly = pose.body.frame().up * -1.0;
        if faces_viewer(belly) != front {
            return;
        }
        let radius = FOOT_RADIUS * pose.scale * (1.0 - pose.tuck);
        for foot in pose.feet {
            canvas.fill_circle(self.0.at(foot), radius, ORANGE, 1.0);
        }
    }

    /// Проплешина на боку, обращённом к зрителю.
    fn bald(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.0, self.0.pose);
        let bald = pose.look.bald;
        if bald <= 0.0 {
            return;
        }
        let body = &pose.body;
        let frame = Frame::new(body.forward.y.atan2(body.forward.x), body.pitch, 0.0);
        let side = if toward_viewer(frame.right) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let [along, out, up] = BALD_AT;
        let at = frame.point(
            body.center,
            along * body.radii[0],
            side * out * body.radii[1],
            up * body.radii[2],
        );
        let r = (BALD_RADIUS[0] + BALD_RADIUS[1] * bald) * pose.scale;
        canvas.fill_ellipse(pen.at(at), Vec2::new(r, r * 0.8), 0.0, SKIN, 1.0);
    }

    /// Петля на шее: кольцо из капсул сбоку от неё, у середины.
    fn knot(&self, canvas: &mut Canvas, color: Color, radius: f32) {
        let (pen, pose) = (self.0, self.0.pose);
        if pose.antic.knot <= 0.0 {
            return;
        }
        let ring = KNOT_RADIUS * pose.scale * pose.antic.knot.min(1.0);
        let center = pen.at(pose.neck[KNOT_AT]) + Vec2::new(ring, 0.0);
        let at = |i: usize| {
            let angle = i as f32 * std::f32::consts::TAU / KNOT_SEGMENTS as f32;
            center + Vec2::from_angle(angle) * ring
        };
        for i in 0..KNOT_SEGMENTS {
            canvas.fill_capsule(at(i), at(i + 1), radius, radius, color, 1.0);
        }
    }

    /// Крылья из двух перьев веером; далёшие — за телом, ближние — поверх.
    fn wings(&self, canvas: &mut Canvas, near: bool) {
        let (pen, pose, s) = (self.0, self.0.pose, self.0.pose.scale);
        let (open, phase) = (pose.antic.wings.clamp(0.0, 1.0), pose.antic.flap);
        if open <= 0.0 {
            return;
        }
        let body = &pose.body;
        let frame = Frame::new(body.forward.y.atan2(body.forward.x), body.pitch, 0.0);
        let flap = (phase * std::f32::consts::TAU).sin() * FLAP_SWING * open;
        let length = WING_LENGTH * body.radii[0] * open;
        let [along, out, up] = WING_ROOT;
        for side in [-1.0, 1.0] {
            if (toward_viewer(frame.right * side) > WING_NEAR) != near {
                continue;
            }
            let root = frame.point(
                body.center,
                along * body.radii[0],
                side * out * body.radii[1],
                up * body.radii[2],
            );
            let tips = WING_FEATHERS.map(|(lift, back, reach)| {
                let angle = WING_REST_LIFT * open + lift + flap;
                let tip = frame.point(
                    root,
                    -back * length,
                    side * angle.cos() * length * reach,
                    angle.sin() * length * reach,
                );
                pen.at(tip)
            });
            let a = pen.at(root);
            let (r0, r1) = (WING_RADII[0] * s, WING_RADII[1] * s);
            let grow = OUTLINE_WIDTH * s;
            for tip in tips {
                canvas.fill_capsule(a, tip, r0 + grow, r1 + grow, OUTLINE, 1.0);
            }
            for tip in tips {
                canvas.fill_capsule(a, tip, r0, r1, WHITE, 1.0);
            }
        }
    }

    fn beak(&self, canvas: &mut Canvas) {
        let (pen, pose, s) = (self.0, self.0.pose, self.0.pose.scale);
        let frame = pose.head_frame();
        let base = pose.head.center + frame.forward * (pose.head.radius * BEAK_BASE);
        let tip = pose.muzzle_tip();
        let gape = (pose.antic.mouth.clamp(0.0, 1.0) * GAPE).max(if pose.antic.tongue {
            TONGUE_GAPE
        } else {
            0.0
        });
        if gape <= 0.0 {
            canvas.fill_capsule(
                pen.at(base),
                pen.at(tip),
                BEAK_RADII[0] * s,
                BEAK_RADII[1] * s,
                ORANGE,
                1.0,
            );
        } else {
            self.gaping(canvas, base, (tip - base).length(), gape);
        }
        let mud = pose.look.snout;
        if mud > 0.0 {
            let (from, to) = (pen.at(base.lerp(tip, MUD_FROM)), pen.at(tip));
            let r = BEAK_RADII[1] * s * MUD_WIDTH;
            canvas.fill_capsule(from, to, r, r, MUD, mud);
        }
    }

    /// Две половинки клюва углом; между ними нутро и язык.
    fn gaping(&self, canvas: &mut Canvas, base: Vec3, length: f32, gape: f32) {
        let (pen, pose, s) = (self.0, self.0.pose, self.0.pose.scale);
        let frame = pose.head_frame();
        let half = |sign: f32| {
            let dir = frame.forward * gape.cos() + frame.up * (sign * gape.sin());
            base + dir * length
        };
        let (upper, lower) = (half(1.0), half(-1.0));
        let radii = HALF_RADII.map(|r| r * s);
        let base2 = pen.at(base);
        canvas.fill_capsule(base2, pen.at(lower), radii[0], radii[1], ORANGE, 1.0);
        canvas.fill_capsule(
            base2,
            pen.at(base.lerp(upper.lerp(lower, 0.5), 0.9)),
            radii[0],
            radii[1],
            MOUTH,
            1.0,
        );
        if pose.antic.tongue {
            let down = frame.right * (TONGUE_AT[0] * pose.head.radius)
                + frame.up * (-TONGUE_AT[1] * pose.head.radius);
            let root = base.lerp(lower, 0.6);
            canvas.fill_capsule(
                pen.at(root),
                pen.at(root + down),
                TONGUE_RADII[0] * s,
                TONGUE_RADII[1] * s,
                TONGUE,
                1.0,
            );
        }
        canvas.fill_capsule(base2, pen.at(upper), radii[0], radii[1], ORANGE, 1.0);
    }

    fn eyes(&self, canvas: &mut Canvas) {
        let pose = self.0.pose;
        let head = &pose.head;
        let frame = pose.head_frame();
        if head.eyes_open < 0.5 || turned_away(frame.forward) {
            return;
        }
        for side in [-1.0, 1.0] {
            let offset = (frame.right * (EYE_AT[0] * side)
                + frame.forward * EYE_AT[1]
                + frame.up * EYE_AT[2])
                * head.radius;
            // Только глаз на видимой стороне головы
            if !faces_viewer(offset) {
                continue;
            }
            let (at, s) = (self.0.on_head(offset), pose.scale);
            match pose.antic.eyes {
                Eyes::Normal => canvas.fill_circle(at, EYE_RADIUS * s, INK, 1.0),
                Eyes::Crossed => cross(canvas, at, EYE_RADIUS * CROSS_REACH * s, CROSS_WIDTH * s),
                Eyes::Wide => {
                    canvas.fill_circle(at, (WIDE_EYE[0] + OUTLINE_WIDTH) * s, OUTLINE, 1.0);
                    canvas.fill_circle(at, WIDE_EYE[0] * s, WHITE, 1.0);
                    canvas.fill_circle(at, WIDE_EYE[1] * s, INK, 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::creature::test_support::{
        self, HEADINGS, SCALE, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Pose, RigKind};

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Goose, heading)
    }

    #[test]
    fn draws_inside_the_rect_and_nothing_outside() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            assert!(solid(&pixels, WHITE) > 600, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn beak_and_both_feet_are_orange() {
        let (pixels, ..) = render(&pose(Vec2::new(1.0, 0.0)));
        let feet = 2.0 * std::f32::consts::PI * (FOOT_RADIUS * SCALE).powi(2);
        // Мозаика размывает края: чистого цвета остаётся меньше площади кругов
        assert!(solid(&pixels, ORANGE) as f32 > feet * 0.4);
    }

    #[test]
    fn head_not_body_decides_beak_and_eyes() {
        let mut goose = pose(Vec2::new(0.0, -1.0));
        let (back, ..) = render(&goose);
        assert_eq!(solid(&back, INK), 0);
        // Тело спиной, голова обернулась к зрителю: клюв поверх головы
        goose.head.dir = Vec2::new(0.0, 1.0);
        let (turned, ..) = render(&goose);
        assert!(solid(&turned, ORANGE) > solid(&back, ORANGE));
        assert!(solid(&turned, INK) > 0);
        // Голова через плечо от зрителя: ни глаза
        goose.head.dir = Vec2::new(0.6, -0.8);
        let (over_shoulder, ..) = render(&goose);
        assert_eq!(solid(&over_shoulder, INK), 0);
    }

    fn front() -> Pose {
        pose(Vec2::new(0.0, 1.0))
    }

    #[test]
    fn crossed_eyes_have_no_whites_and_wide_ones_more() {
        let mut goose = front();
        let (normal, ..) = render(&goose);
        goose.antic.eyes = Eyes::Wide;
        let (wide, ..) = render(&goose);
        goose.antic.eyes = Eyes::Crossed;
        let (crossed, ..) = render(&goose);
        // Белок на белой голове виден только обводкой и зрачком
        assert_ne!(wide, normal);
        assert_ne!(crossed, normal);
        assert!(solid(&wide, INK) > 0 && solid(&crossed, INK) > 0);
    }

    #[test]
    fn open_wings_add_white_on_the_sides_inside_the_rect() {
        for heading in HEADINGS {
            let mut goose = pose(heading);
            let (folded, ..) = render(&goose);
            goose.antic.wings = 1.0;
            goose.antic.flap = 0.25;
            let (open, w, h) = render(&goose);
            let (before, after) = (opaque(&folded), opaque(&open));
            assert!(after > before + 100, "{heading:?} {before} {after}");
            assert_eq!(outside_rect(&open, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn mouth_tongue_bald_and_mud_show() {
        let mut goose = front();
        let (plain, ..) = render(&goose);
        goose.antic.mouth = 1.0;
        let (open, ..) = render(&goose);
        assert_ne!(open, plain);
        goose.antic.tongue = true;
        let (tongue, ..) = render(&goose);
        assert!(solid(&tongue, TONGUE) > 0);
        assert_eq!((solid(&plain, SKIN), solid(&plain, MUD)), (0, 0));
        goose.look.bald = 0.2;
        goose.look.snout = 1.0;
        let (marked, ..) = render(&goose);
        assert!(solid(&marked, SKIN) > 0 && solid(&marked, MUD) > 0);
        goose.look.bald = 1.0;
        let (bare, ..) = render(&goose);
        assert!(solid(&bare, SKIN) > solid(&marked, SKIN));
    }

    #[test]
    fn knot_adds_a_loop_to_the_neck() {
        let mut goose = front();
        let (plain, ..) = render(&goose);
        goose.antic.knot = 1.0;
        let (knotted, w, h) = render(&goose);
        assert!(solid(&knotted, WHITE) > solid(&plain, WHITE));
        assert_eq!(outside_rect(&knotted, w, h), 0);
    }

    #[test]
    fn tucked_feet_disappear() {
        let mut tucked = pose(Vec2::new(1.0, 0.0));
        let (standing, ..) = render(&tucked);
        tucked.tuck = 1.0;
        let (sleeping, ..) = render(&tucked);
        assert!(solid(&sleeping, ORANGE) < solid(&standing, ORANGE));
    }

    #[test]
    fn outline_is_thin_and_light() {
        let (pixels, ..) = render(&pose(Vec2::new(1.0, 0.0)));
        let outline = pixels
            .iter()
            .filter(|p| **p >> 24 == 255 && (0xA0..0xF0).contains(&(**p & 255)))
            .count();
        assert!(outline > 30 && outline < solid(&pixels, WHITE) / 3);
    }
}
