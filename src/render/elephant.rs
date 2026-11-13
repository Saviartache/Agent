//! Слонёнок: серо-голубое пухлое тело с головой одним силуэтом, ноги-тумбы,
//! уши-лопухи, гибкий хобот и два глаза со зрачком в камеру. Детали головы ложатся
//! за неё или поверх, как у свинки; в каждом слое порядок один: уши, хобот, глаза.

use std::f32::consts::TAU;

use super::creature::{
    INK, MOUTH, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer, toward_viewer,
    turned_away,
};
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

const HIDE: Color = Color(0x93_A8_C2);
/// Уши и хвост.
const EAR: Color = Color(0x7F_95_B1);
/// Изнанка уха: видна, когда слонёнок смотрит на зрителя.
const INNER: Color = Color(0xE0_A4_B0);
const LEG: Color = Color(0x71_87_A3);
const TOENAIL: Color = Color(0xC2_CF_DE);
const TUFT: Color = Color(0x5B_6F_8C);
/// Веки светлее тела: закрытый глаз не сливается ни с ним, ни с ушами.
const LID: Color = Color(0xBC_CB_DC);

// Размеры — design px, умножаются на `pose.scale`; доли — от радиуса головы.
/// Шеи почти нет: голова сидит на теле.
const NECK_RADIUS: f32 = 7.5;
/// Бедро от центра тела в долях его полуосей: по ходу, вбок и вниз.
const HIP: [f32; 3] = [0.55, 0.5, 0.5];
/// Нога-тумба: у ступни шире, чем у бедра.
const LEG_RADII: [f32; 2] = [3.9, 3.5];
const LEG_MAX: f32 = 11.0;
/// Ногти на ступне: вбок и к зрителю в долях её радиуса.
const TOENAILS: [[f32; 2]; 2] = [[-0.45, 0.55], [0.45, 0.55]];
const TOENAIL_RADIUS: f32 = 1.3;
/// Хвост от центра тела в долях его полуосей: назад и вверх, от корня к кончику.
const TAIL: [[f32; 2]; 2] = [[0.95, 0.35], [1.2, -0.25]];
const TAIL_RADIUS: f32 = 1.1;
const TUFT_RADIUS: f32 = 1.7;
const SIDES: [f32; 2] = [-1.0, 1.0];
/// Ухо: вбок, вперёд, вверх — корень, кончик висящего и кончик расправленного.
const EAR_ROOT: [f32; 3] = [0.75, -0.25, 0.3];
const EAR_HANG: [f32; 3] = [1.0, -0.55, -0.45];
const EAR_SPREAD: [f32; 3] = [1.5, 0.05, 0.2];
const EAR_RADII: [f32; 2] = [4.4, 6.8];
/// Изнанка уха: доля радиусов и отрезок от корня к кончику.
const INNER_SIZE: f32 = 0.55;
const INNER_SPAN: [f32; 2] = [0.25, 0.9];
const INNER_FACING: f32 = 0.45;
/// Уши хлопают: разница высот задних ступней и подъём тела, при которых расправлены целиком.
const EAR_STRIDE: f32 = 7.0;
const EAR_HOP: f32 = 6.0;
const FLAP_BASE: f32 = 0.6;
/// Хобот: точек от корня до кончика, радиусы и как далеко вперёд он выходит из морды
/// в долях длины, прежде чем свернуть к кончику.
const TRUNK_POINTS: usize = 9;
const TRUNK_RADII: [f32; 2] = [3.3, 1.8];
const TRUNK_BEND: f32 = 0.45;
/// Глаз: вбок, вперёд, вверх.
const EYE_AT: [f32; 3] = [0.5, 0.65, 0.35];
const EYE_RADIUS: f32 = 3.1;
const PUPIL_RADIUS: f32 = 1.4;
/// Рот под хоботом: вперёд и вверх; полуоси, design px.
const MOUTH_AT: [f32; 2] = [0.75, -0.6];
/// Шире корня хобота: видно по бокам от него.
const MOUTH_RADII: [f32; 2] = [4.4, 0.9];
const MOUTH_OPEN: f32 = 1.8;
const TONGUE_SIDE: f32 = 0.35;
const TONGUE_RADII: [f32; 2] = [1.5, 1.2];
const CROSS_REACH: f32 = 0.65;
const CROSS_WIDTH: f32 = 0.6;
const WIDE_EYE: f32 = 1.3;
const WIDE_PUPIL: f32 = 0.65;
/// Отвернулся — глаза уходят к центру головы и прячутся за затылком.
const EYE_SINK: f32 = 0.7;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let elephant = Elephant {
        pen,
        frame: pen.pose.head_frame(),
    };
    elephant.legs(canvas, false);
    elephant.details(canvas, false);
    elephant.hide(canvas);
    elephant.legs(canvas, true);
    elephant.details(canvas, true);
}

struct Elephant<'a> {
    pen: &'a Pen<'a>,
    frame: Frame,
}

impl Elephant<'_> {
    /// Смещение от центра головы: доли её радиуса вбок, вперёд и вверх.
    fn on_head(&self, at: [f32; 3], side: f32) -> Vec3 {
        (self.frame.right * (at[0] * side) + self.frame.forward * at[1] + self.frame.up * at[2])
            * self.pen.pose.head.radius
    }

    fn eye_at(&self, side: f32) -> Vec3 {
        let away = (-toward_viewer(self.frame.forward)).clamp(0.0, 1.0);
        self.on_head(EYE_AT, side) * (1.0 - EYE_SINK * away)
    }

    /// Всё серое одним цветом: швов между телом и головой нет.
    fn hide(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, HIDE, 1.0);
        let radius = NECK_RADIUS * pose.scale;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, HIDE, 1.0);
        }
        canvas.fill_circle(pen.at(pose.head.center), pose.head.radius, HIDE, 1.0);
    }

    /// Ноги по одну сторону от серого: `front` — те, что торчат к зрителю (на боку, на спине).
    fn legs(&self, canvas: &mut Canvas, front: bool) {
        let (pen, pose, body) = (self.pen, self.pen.pose, &self.pen.pose.body);
        let out = 1.0 - pose.tuck;
        let k = pose.scale * (2.0 * out).min(1.0);
        let frame = body.frame();
        let forward = frame.forward * (body.radii[0] * HIP[0]);
        let right = frame.right * (body.radii[1] * HIP[1]);
        let down = frame.up * (-body.radii[2] * HIP[2]);
        let belly_front = faces_viewer(down);
        if belly_front != front {
            return;
        }
        let pairs = [(pose.feet, -1.0), (pose.fore.unwrap_or(pose.feet), 1.0)];
        for (feet, along) in pairs {
            for (foot, side) in feet.into_iter().zip(SIDES) {
                let hip = body.center + forward * along + right * side + down;
                let reach = (foot - hip).length().max(f32::EPSILON);
                let stretch = (LEG_MAX * pose.scale / reach).min(1.0);
                let sole = hip.lerp(foot, out * stretch);
                let (a, b) = (pen.at(sole), pen.at(hip));
                canvas.fill_capsule(a, b, LEG_RADII[0] * k, LEG_RADII[1] * k, LEG, 1.0);
                // Ногти только у стоящей ноги: на боку и поджатой их не видно
                if belly_front || out < 1.0 {
                    continue;
                }
                for [x, y] in TOENAILS {
                    let at = a + Vec2::new(x, y) * (LEG_RADII[0] * k);
                    canvas.fill_circle(at, TOENAIL_RADIUS * k, TOENAIL, 1.0);
                }
            }
        }
    }

    /// Детали по одну сторону от серого: `front` — те, что поверх него.
    fn details(&self, canvas: &mut Canvas, front: bool) {
        if (self.pen.pose.body.forward.y < 0.0) == front {
            self.tail(canvas);
        }
        let away = turned_away(self.frame.forward);
        let eye_front = |side| !away && faces_viewer(self.eye_at(side));
        for side in SIDES {
            let ear_front = (away || eye_front(side)) && faces_viewer(self.on_head(EAR_ROOT, side));
            if ear_front == front {
                self.ear(canvas, side);
            }
        }
        // Хобот за головой, только когда морда отвернулась; кивок вниз его не прячет
        let ahead = Vec3::on_ground(self.pen.pose.head.dir, 0.0);
        if turned_away(ahead) != front {
            self.mouth(canvas);
            self.trunk(canvas);
        }
        // Глаз за головой не рисуется: из-за широкого лба он торчал бы бровью
        if !front {
            return;
        }
        for side in SIDES {
            if eye_front(side) {
                self.eye(canvas, side);
            }
        }
    }

    fn tail(&self, canvas: &mut Canvas) {
        let (pen, body) = (self.pen, &self.pen.pose.body);
        let frame = body.frame();
        let back = frame.forward * -body.radii[0];
        let up = frame.up * body.radii[2];
        let [root, tip] = TAIL.map(|[b, u]| pen.at(body.center + back * b + up * u));
        let s = pen.pose.scale;
        canvas.fill_capsule(root, tip, TAIL_RADIUS * s, TAIL_RADIUS * s, EAR, 1.0);
        canvas.fill_circle(tip, TUFT_RADIUS * s, TUFT, 1.0);
    }

    /// Насколько уши расправлены: на бегу хлопают в такт шагу, в прыжке от радости
    /// и по сценке (`Antic::wings`, взмах — `flap`).
    fn spread(&self) -> f32 {
        let (pose, s) = (self.pen.pose, self.pen.pose.scale);
        let stride = (pose.feet[0].z - pose.feet[1].z).abs() / (EAR_STRIDE * s);
        let hop = pose.rise() / (EAR_HOP * s);
        let antic = &pose.antic;
        let flap = antic.wings * (FLAP_BASE + (1.0 - FLAP_BASE) * (TAU * antic.flap).sin());
        stride.max(hop).max(flap).clamp(0.0, 1.0)
    }

    fn ear(&self, canvas: &mut Canvas, side: f32) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let spread = self.spread();
        let tip: [f32; 3] =
            std::array::from_fn(|i| EAR_HANG[i] + (EAR_SPREAD[i] - EAR_HANG[i]) * spread);
        let root = pen.on_head(self.on_head(EAR_ROOT, side));
        let tip = pen.on_head(self.on_head(tip, side));
        canvas.fill_capsule(root, tip, EAR_RADII[0] * s, EAR_RADII[1] * s, EAR, 1.0);
        // По взгляду на земле: задранная голова изнанку не показывает
        let ahead = Vec3::on_ground(self.pen.pose.head.dir, 0.0);
        if toward_viewer(ahead) < INNER_FACING {
            return;
        }
        let [a, b] = INNER_SPAN.map(|t| root.lerp(tip, t));
        let [ra, rb] = EAR_RADII.map(|r| r * INNER_SIZE * s);
        canvas.fill_capsule(a, b, ra, rb, INNER, 1.0);
    }

    /// Хобот выходит из морды вперёд и гнётся к кончику, который живёт на пружине.
    fn trunk(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let Some([root, tip]) = pose.trunk else {
            return;
        };
        let control = root + self.frame.forward * ((tip - root).length() * TRUNK_BEND);
        let last = (TRUNK_POINTS - 1) as f32;
        let point = |i: usize| {
            let t = i as f32 / last;
            pen.at(root.lerp(control, t).lerp(control.lerp(tip, t), t))
        };
        let radius = |i: usize| {
            let t = i as f32 / last;
            (TRUNK_RADII[0] + (TRUNK_RADII[1] - TRUNK_RADII[0]) * t) * pose.scale
        };
        for i in 1..TRUNK_POINTS {
            canvas.fill_capsule(point(i - 1), point(i), radius(i - 1), radius(i), HIDE, 1.0);
        }
    }

    /// Тёмный рот под хоботом и язык набок.
    fn mouth(&self, canvas: &mut Canvas) {
        let (pen, antic, s) = (self.pen, &self.pen.pose.antic, self.pen.pose.scale);
        if antic.mouth <= 0.0 && !antic.tongue {
            return;
        }
        let at = |side, along, up| pen.on_head(self.on_head([side, along, up], 1.0));
        let center = at(0.0, MOUTH_AT[0], MOUTH_AT[1]);
        let open = antic.mouth.clamp(0.0, 1.0);
        let radii = Vec2::new(MOUTH_RADII[0] * s, (MOUTH_RADII[1] + MOUTH_OPEN * open) * s);
        if open > 0.0 {
            canvas.fill_ellipse(center, radii, 0.0, MOUTH, 1.0);
        }
        if antic.tongue {
            let hang = at(TONGUE_SIDE, MOUTH_AT[0], MOUTH_AT[1] - 0.3);
            let [a, b] = TONGUE_RADII.map(|r| r * s);
            canvas.fill_capsule(center, hang, a, b, TONGUE, 1.0);
        }
    }

    /// Белый шар с маленьким зрачком, глядящим в камеру; закрытый — веко.
    fn eye(&self, canvas: &mut Canvas, side: f32) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let center = pen.on_head(self.eye_at(side));
        let mut radius = EYE_RADIUS * s;
        if pen.pose.head.eyes_open < 0.5 {
            canvas.fill_circle(center, radius, LID, 1.0);
            return;
        }
        let eyes = pen.pose.antic.eyes;
        if eyes == Eyes::Crossed {
            cross(canvas, center, radius * CROSS_REACH, CROSS_WIDTH * s);
            return;
        }
        let mut pupil = PUPIL_RADIUS * s;
        if eyes == Eyes::Wide {
            radius *= WIDE_EYE;
            pupil *= WIDE_PUPIL;
        }
        canvas.fill_circle(center, radius + OUTLINE_WIDTH * s, OUTLINE, 1.0);
        canvas.fill_circle(center, radius, WHITE, 1.0);
        canvas.fill_circle(center, pupil, INK, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::creature::test_support::{
        self, HEADINGS, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Pose, RigKind};

    const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
    const AWAY: Vec2 = Vec2::new(0.0, -1.0);
    const RIGHT: Vec2 = Vec2::new(1.0, 0.0);

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Elephant, heading)
    }

    #[test]
    fn draws_inside_the_rect_and_nothing_outside() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            assert!(solid(&pixels, HIDE) > 150, "{heading:?}");
            assert!(opaque(&pixels) > 1200, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn stands_on_four_pillars_with_toenails() {
        let elephant = pose(RIGHT);
        assert!(elephant.fore.is_some());
        let mut tucked = elephant.clone();
        tucked.tuck = 1.0;
        let (standing, ..) = render(&elephant);
        let (sleeping, ..) = render(&tucked);
        assert!(solid(&standing, LEG) > solid(&sleeping, LEG));
        assert!(solid(&standing, TOENAIL) > 0);
        assert_eq!(solid(&sleeping, TOENAIL), 0);
    }

    #[test]
    fn trunk_hangs_ahead_of_the_head_and_below_it() {
        let elephant = pose(RIGHT);
        let [root, tip] = elephant.trunk.unwrap();
        assert_eq!(elephant.muzzle_tip(), tip);
        assert!(tip.z < root.z && tip.z > 0.0);
        assert!(tip.x > elephant.head.center.x);
    }

    #[test]
    fn ears_spread_wider_when_flapping() {
        let mut elephant = pose(TOWARD);
        let (calm, ..) = render(&elephant);
        elephant.antic.wings = 1.0;
        elephant.antic.flap = 0.25;
        let (flapping, ..) = render(&elephant);
        assert!(solid(&flapping, EAR) > solid(&calm, EAR));
        assert!(solid(&calm, INNER) > 0, "изнанка видна спереди");
        let (back, ..) = render(&pose(AWAY));
        assert_eq!(solid(&back, INNER), 0);
    }

    /// Пиксели, близкие к цвету: мелкие детали мозаика не оставляет чистыми.
    fn near(pixels: &[u32], color: Color) -> usize {
        let channels = |c: u32| [c >> 16 & 0xFF, c >> 8 & 0xFF, c & 0xFF];
        let want = channels(color.0);
        pixels
            .iter()
            .filter(|p| **p >> 24 == 0xFF)
            .filter(|p| {
                let got = channels(**p);
                (0..3).all(|i| got[i].abs_diff(want[i]) <= 16)
            })
            .count()
    }

    #[test]
    fn eyes_show_only_from_the_front_and_close_into_lids() {
        let mut front = pose(TOWARD);
        let (open, ..) = render(&front);
        let (back, ..) = render(&pose(AWAY));
        assert!(near(&open, WHITE) > 20, "{}", near(&open, WHITE));
        assert!(solid(&open, INK) > 0);
        assert_eq!(solid(&back, INK), 0);
        front.head.eyes_open = 0.0;
        let (closed, ..) = render(&front);
        assert_eq!(near(&closed, WHITE), 0);
        assert!(near(&closed, LID) > 20, "{}", near(&closed, LID));
        front.head.eyes_open = 1.0;
        front.antic.eyes = Eyes::Crossed;
        let (crossed, ..) = render(&front);
        assert_eq!(near(&crossed, WHITE), 0);
    }

    #[test]
    fn mouth_and_tongue_show_from_the_front() {
        let mut elephant = pose(TOWARD);
        let (plain, ..) = render(&elephant);
        elephant.antic.mouth = 1.0;
        elephant.antic.tongue = true;
        let (open, ..) = render(&elephant);
        assert!(near(&open, MOUTH) > near(&plain, MOUTH));
        assert!(near(&open, TONGUE) > 0);
    }
}
