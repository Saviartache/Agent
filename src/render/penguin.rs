//! Пингвин: тёмное яйцо торчком с головой одним силуэтом, белый живот спереди,
//! ласты по бокам, оранжевые клюв (гусиный) и плоские лапки, глаза в белых колечках.

use super::creature::{INK, Pen, WHITE, cross, faces_viewer, toward_viewer, turned_away};
use super::goose::{ORANGE, beak};
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

/// Спина и голова: темнее всего, но не чернила.
const BACK: Color = Color(0x2E_34_45);
/// Ласты светлее спины: видны поверх неё.
const FLIPPER: Color = Color(0x45_4E_63);

// Доли — от полуосей тела (по росту, вбок, по глубине) или радиуса головы.
/// Живот — эллипсоид внутри тела, сдвинутый к груди: доли полуосей и сдвиг вниз и к груди.
const BELLY_RADII: [f32; 3] = [0.8, 0.78, 0.55];
const BELLY_AT: [f32; 2] = [-0.1, 0.4];
/// Живот виден, пока грудь не отвернулась от зрителя дальше этого.
const BELLY_FACING: f32 = -0.2;
/// Ласт: корень (по росту, вбок, к спине), длина в продольных полуосях.
const FLIPPER_ROOT: [f32; 3] = [0.45, 0.85, 0.35];
const FLIPPER_LENGTH: f32 = 0.85;
/// Угол от тела, рад: прижат, оттопырен целиком, размах взмаха.
const FLIPPER_REST: f32 = 0.2;
const FLIPPER_SPREAD: f32 = 1.3;
const FLIPPER_SWING: f32 = 0.5;
/// Ласт ближе к зрителю этого порога рисуется поверх тела.
const FLIPPER_NEAR: f32 = -0.1;
/// Глаз: вбок, вперёд, вверх.
const EYE_AT: [f32; 3] = [0.5, 0.5, 0.3];
/// Лёжа лапы — со стороны живота, как у гуся.
const LYING: f32 = 0.5;

// Размеры — design px, умножаются на `pose.scale`.
const NECK_RADIUS: f32 = 5.0;
const FLIPPER_RADII: [f32; 2] = [2.6, 1.3];
/// Плоская лапка: вдоль хода и поперёк; носок впереди точки шага.
const FOOT_RADII: [f32; 2] = [2.8, 1.8];
const FOOT_AHEAD: f32 = 1.2;
const EYE_RING: f32 = 1.9;
const PUPIL: f32 = 1.1;
const WIDE_EYE: f32 = 2.6;
const CLOSED_EYE: [f32; 2] = [1.6, 0.8];
const CROSS_REACH: f32 = 1.2;
const CROSS_WIDTH: f32 = 0.55;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let penguin = Penguin {
        pen,
        frame: pen.pose.body.frame(),
    };
    let beak_front = !turned_away(pen.pose.head_frame().forward);
    penguin.feet(canvas, false);
    penguin.flippers(canvas, false);
    if !beak_front {
        beak(canvas, pen);
    }
    penguin.dark(canvas);
    penguin.belly(canvas);
    penguin.flippers(canvas, true);
    penguin.feet(canvas, true);
    if beak_front {
        beak(canvas, pen);
    }
    penguin.eyes(canvas);
}

struct Penguin<'a> {
    pen: &'a Pen<'a>,
    /// Оси тела: вперёд — к макушке, вверх — к спине.
    frame: Frame,
}

impl Penguin<'_> {
    /// Спина, шея и голова одним цветом: швов нет.
    fn dark(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, BACK, 1.0);
        let radius = NECK_RADIUS * pose.scale;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, BACK, 1.0);
        }
        canvas.fill_circle(pen.at(pose.head.center), pose.head.radius, BACK, 1.0);
    }

    /// Белый живот поверх тёмного, пока грудь смотрит к зрителю; обводки не нужно —
    /// он целиком внутри силуэта.
    fn belly(&self, canvas: &mut Canvas) {
        let (body, frame) = (&self.pen.pose.body, &self.frame);
        let chest = frame.up * -1.0;
        if toward_viewer(chest) < BELLY_FACING {
            return;
        }
        let [along, out] = BELLY_AT;
        let center = frame.point(
            body.center,
            along * body.radii[0],
            0.0,
            -out * body.radii[2],
        );
        let radii = std::array::from_fn(|i| body.radii[i] * BELLY_RADII[i]);
        let oval = self.pen.ellipsoid(center, frame, radii);
        canvas.fill_ellipse(oval.center, oval.radii, oval.rotation, WHITE, 1.0);
    }

    /// Ласты висят вдоль боков и оттопыриваются на `antic.wings`; дальний — за телом.
    fn flippers(&self, canvas: &mut Canvas, near: bool) {
        let (pen, pose, s) = (self.pen, self.pen.pose, self.pen.pose.scale);
        let (body, frame) = (&pose.body, &self.frame);
        let open = pose.antic.wings.clamp(0.0, 1.0);
        let flap = (pose.antic.flap * std::f32::consts::TAU).sin() * FLIPPER_SWING * open;
        let angle = FLIPPER_REST + FLIPPER_SPREAD * open + flap;
        let length = FLIPPER_LENGTH * body.radii[0];
        let [along, out, back] = FLIPPER_ROOT;
        for side in [-1.0, 1.0] {
            if (toward_viewer(frame.right * side) > FLIPPER_NEAR) != near {
                continue;
            }
            let root = frame.point(
                body.center,
                along * body.radii[0],
                side * out * body.radii[1],
                back * body.radii[2],
            );
            let tip = frame.point(
                root,
                -angle.cos() * length,
                side * angle.sin() * length,
                0.0,
            );
            let [r0, r1] = FLIPPER_RADII.map(|r| r * s);
            canvas.fill_capsule(pen.at(root), pen.at(tip), r0, r1, FLIPPER, 1.0);
        }
    }

    /// Лапки по одну сторону от тела: стоя — ближние к зрителю спереди,
    /// лёжа — когда живот смотрит к зрителю.
    fn feet(&self, canvas: &mut Canvas, front: bool) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let k = pose.scale * (1.0 - pose.tuck);
        if k <= 0.0 {
            return;
        }
        let body = &pose.body;
        let ahead = Vec3::on_ground(body.forward, 0.0);
        let flat = Frame::new(body.forward.y.atan2(body.forward.x), 0.0, 0.0);
        let lying = body.roll.cos() < LYING;
        for foot in pose.feet {
            let is_front = if lying {
                faces_viewer(self.frame.up * -1.0)
            } else {
                faces_viewer(Vec3::on_ground(foot.ground() - body.center.ground(), 0.0))
            };
            if is_front != front {
                continue;
            }
            let center = foot + ahead * (FOOT_AHEAD * pose.scale);
            let radii = [FOOT_RADII[0] * k, FOOT_RADII[1] * k, 0.0];
            let oval = pen.ellipsoid(center, &flat, radii);
            canvas.fill_ellipse(oval.center, oval.radii, oval.rotation, ORANGE, 1.0);
        }
    }

    /// Глаза в белых колечках — на тёмной голове видны только так.
    fn eyes(&self, canvas: &mut Canvas) {
        let (pen, pose, s) = (self.pen, self.pen.pose, self.pen.pose.scale);
        let frame = pose.head_frame();
        if turned_away(frame.forward) {
            return;
        }
        for side in [-1.0, 1.0] {
            let offset = (frame.right * (EYE_AT[0] * side)
                + frame.forward * EYE_AT[1]
                + frame.up * EYE_AT[2])
                * pose.head.radius;
            if !faces_viewer(offset) {
                continue;
            }
            let at = pen.on_head(offset);
            if pose.head.eyes_open < 0.5 {
                let d = Vec2::new(CLOSED_EYE[0] * s, 0.0);
                let r = CLOSED_EYE[1] * s;
                canvas.fill_capsule(at - d, at + d, r, r, WHITE, 1.0);
                continue;
            }
            let ring = if pose.antic.eyes == Eyes::Wide {
                WIDE_EYE
            } else {
                EYE_RING
            };
            canvas.fill_circle(at, ring * s, WHITE, 1.0);
            match pose.antic.eyes {
                Eyes::Crossed => cross(canvas, at, CROSS_REACH * s, CROSS_WIDTH * s),
                _ => canvas.fill_circle(at, PUPIL * s, INK, 1.0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::creature::test_support::{
        self, HEADINGS, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Pose, RigKind};

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Penguin, heading)
    }

    const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
    const AWAY: Vec2 = Vec2::new(0.0, -1.0);

    #[test]
    fn draws_inside_the_rect_dark_with_orange_feet() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            let body = solid(&pixels, BACK) + solid(&pixels, WHITE);
            assert!(solid(&pixels, BACK) > 100 && body > 400, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
            if heading == AWAY {
                continue;
            }
            // Мозаика смешивает мелкие лапки с фоном: оранжевое, а не ровно ORANGE
            let orange = pixels.iter().filter(|p| {
                let [r, g, b] = [**p >> 16 & 255, **p >> 8 & 255, **p & 255];
                **p >> 24 == 255 && r > 200 && (100..200).contains(&g) && b < 90
            });
            assert!(orange.count() > 4, "{heading:?}");
        }
    }

    #[test]
    fn white_belly_faces_the_viewer_only() {
        let (front, ..) = render(&pose(TOWARD));
        let (side, ..) = render(&pose(Vec2::new(1.0, 0.0)));
        let (back, ..) = render(&pose(AWAY));
        assert!(solid(&front, WHITE) > solid(&side, WHITE));
        assert!(solid(&side, WHITE) > 50);
        assert_eq!(solid(&back, WHITE), 0);
    }

    #[test]
    fn spread_flippers_stick_out_inside_the_rect() {
        for heading in HEADINGS {
            let mut penguin = pose(heading);
            let (folded, ..) = render(&penguin);
            penguin.antic.wings = 1.0;
            let (open, w, h) = render(&penguin);
            assert_ne!(open, folded, "{heading:?}");
            assert_eq!(outside_rect(&open, w, h), 0, "{heading:?}");
            // В профиль ласт смотрит на зрителя и не выходит за силуэт
            if heading.y != 0.0 {
                assert!(opaque(&open) > opaque(&folded), "{heading:?}");
            }
        }
    }

    #[test]
    fn eyes_close_and_go_crossed() {
        let mut penguin = pose(TOWARD);
        let (open, ..) = render(&penguin);
        assert!(solid(&open, INK) > 0);
        penguin.antic.eyes = Eyes::Crossed;
        let (crossed, ..) = render(&penguin);
        assert_ne!(crossed, open);
        penguin.head.eyes_open = 0.0;
        let (closed, ..) = render(&penguin);
        assert_eq!(solid(&closed, INK), 0);
    }

    #[test]
    fn tucked_feet_disappear() {
        let mut penguin = pose(TOWARD);
        penguin.tuck = 1.0;
        let (tucked, ..) = render(&penguin);
        let (standing, ..) = render(&pose(TOWARD));
        assert!(solid(&tucked, ORANGE) < solid(&standing, ORANGE));
    }
}
