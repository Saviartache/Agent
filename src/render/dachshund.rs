//! Такса: длинное рыжее тело, шея, голова и вытянутая морда одним силуэтом,
//! короткие лапы, висячие уши и хвост — тёмнее той же гаммы. Уши болтаются в такт
//! рыси, хвост виляет от радости; глаза — точки чернилами, как у гуся.

use super::creature::{
    INK, MOUTH, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer, turned_away,
};
use super::pig::MUD;
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

const COAT: Color = Color(0xC4_74_3A);
/// Уши и хвост.
const DARK: Color = Color(0x8E_4F_27);
const PAW: Color = Color(0xA3_5A_2C);

// Размеры — design px, умножаются на `pose.scale`; доли — от радиуса головы.
const NECK_RADIUS: f32 = 3.6;
/// Бедро от центра тела в долях его полуосей: по ходу, вбок и вниз.
const HIP: [f32; 3] = [0.62, 0.55, 0.5];
const LEG_RADII: [f32; 2] = [1.9, 2.3];
const LEG_MAX: f32 = 6.0;
/// Хвост в осях тела, design px: корень и кончик (назад, вверх); вбок — виляние.
const TAIL: [[f32; 2]; 2] = [[15.0, 2.5], [22.0, 8.0]];
const TAIL_RADII: [f32; 2] = [1.7, 0.9];
const TAIL_SWING: f32 = 6.0;
const TAIL_DROOP: f32 = 0.9;
const SIDES: [f32; 2] = [-1.0, 1.0];
/// Морда: вперёд (основание и кончик), вверх; радиусы.
const MUZZLE_ALONG: [f32; 2] = [0.5, 1.9];
const MUZZLE_UP: f32 = -0.2;
const MUZZLE_RADII: [f32; 2] = [3.2, 1.9];
const NOSE_RADIUS: f32 = 1.5;
/// Ухо: корень (вбок, вперёд, вверх), длина вниз, радиусы от корня к кончику.
const EAR_ROOT: [f32; 3] = [0.7, -0.4, 0.45];
const EAR_LENGTH: f32 = 1.35;
const EAR_RADII: [f32; 2] = [2.4, 2.1];
/// Ухо на рыси отлетает вперёд-назад на столько радиусов, когда лапа поднята на `FLOP_LIFT`.
const EAR_FLOP: f32 = 0.45;
const FLOP_LIFT: f32 = 3.0;
/// Глаз: вбок, вперёд, вверх.
const EYE_AT: [f32; 3] = [0.45, 0.6, 0.4];
const EYE_RADIUS: f32 = 1.4;
const LID_LENGTH: f32 = 1.3;
const LID_WIDTH: f32 = 0.5;
const CROSS_REACH: f32 = 1.0;
const CROSS_WIDTH: f32 = 0.55;
const WIDE_EYE: [f32; 2] = [2.4, 1.2];
/// Рот под мордой: вперёд, вверх; полуоси.
const MOUTH_AT: [f32; 2] = [1.35, -0.6];
const MOUTH_RADII: [f32; 2] = [2.2, 0.8];
const MOUTH_OPEN: f32 = 1.6;
const TONGUE_RADII: [f32; 2] = [1.3, 1.1];
const MUD_NOSE: f32 = 1.4;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let dog = Dachshund {
        pen,
        frame: pen.pose.head_frame(),
    };
    dog.legs(canvas, false);
    dog.details(canvas, false);
    dog.coat(canvas);
    dog.legs(canvas, true);
    dog.details(canvas, true);
}

struct Dachshund<'a> {
    pen: &'a Pen<'a>,
    frame: Frame,
}

impl Dachshund<'_> {
    /// Смещение от центра головы: доли её радиуса вбок, вперёд и вверх.
    fn on_head(&self, at: [f32; 3], side: f32) -> Vec3 {
        (self.frame.right * (at[0] * side) + self.frame.forward * at[1] + self.frame.up * at[2])
            * self.pen.pose.head.radius
    }

    fn point(&self, at: [f32; 3], side: f32) -> Vec2 {
        self.pen.on_head(self.on_head(at, side))
    }

    /// Тело, шея, голова и морда одним цветом: швов нет.
    fn coat(&self, canvas: &mut Canvas) {
        let (pen, pose, s) = (self.pen, self.pen.pose, self.pen.pose.scale);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, COAT, 1.0);
        let radius = NECK_RADIUS * s;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, COAT, 1.0);
        }
        canvas.fill_circle(pen.at(pose.head.center), pose.head.radius, COAT, 1.0);
        let [base, tip] = MUZZLE_ALONG.map(|along| self.point([0.0, along, MUZZLE_UP], 1.0));
        let [r0, r1] = MUZZLE_RADII.map(|r| r * s);
        canvas.fill_capsule(base, tip, r0, r1, COAT, 1.0);
    }

    /// Лапы по одну сторону от рыжего: `front` — те, что торчат к зрителю.
    fn legs(&self, canvas: &mut Canvas, front: bool) {
        let (pen, pose, body) = (self.pen, self.pen.pose, &self.pen.pose.body);
        let out = 1.0 - pose.tuck;
        let k = pose.scale * (2.0 * out).min(1.0);
        let frame = body.frame();
        let forward = frame.forward * (body.radii[0] * HIP[0]);
        let right = frame.right * (body.radii[1] * HIP[1]);
        let down = frame.up * (-body.radii[2] * HIP[2]);
        if faces_viewer(down) != front {
            return;
        }
        let pairs = [(pose.feet, -1.0), (pose.fore.unwrap_or(pose.feet), 1.0)];
        for (feet, along) in pairs {
            for (foot, side) in feet.into_iter().zip(SIDES) {
                let hip = body.center + forward * along + right * side + down;
                let reach = (foot - hip).length().max(f32::EPSILON);
                let stretch = (LEG_MAX * pose.scale / reach).min(1.0);
                let paw = pen.at(hip.lerp(foot, out * stretch));
                canvas.fill_capsule(
                    paw,
                    pen.at(hip),
                    LEG_RADII[0] * k,
                    LEG_RADII[1] * k,
                    PAW,
                    1.0,
                );
            }
        }
    }

    /// Детали по одну сторону от рыжего: `front` — те, что поверх него.
    fn details(&self, canvas: &mut Canvas, front: bool) {
        if (self.pen.pose.body.forward.y < 0.0) == front {
            self.tail(canvas);
        }
        let away = turned_away(self.frame.forward);
        let eye_front = |side| !away && faces_viewer(self.on_head(EYE_AT, side));
        for side in SIDES {
            let ear_front = (away || eye_front(side)) && faces_viewer(self.on_head(EAR_ROOT, side));
            if ear_front == front {
                self.ear(canvas, side);
            }
        }
        if !front {
            return;
        }
        if faces_viewer(self.frame.forward) {
            self.nose(canvas);
        }
        for side in SIDES {
            if eye_front(side) {
                self.eye(canvas, side);
            }
        }
    }

    /// Хвост торчит вверх-назад и виляет вбок; у спящей лежит.
    fn tail(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let (s, frame) = (pose.scale, pose.body.frame());
        let swing = TAIL_SWING * pose.wag;
        let droop = 1.0 - TAIL_DROOP * pose.tuck;
        let [root, tip] = TAIL.map(|[back, up]| {
            let up = up * droop;
            let aside = swing * (back - TAIL[0][0]) / (TAIL[1][0] - TAIL[0][0]);
            pen.at(frame.point(pose.body.center, -back * s, aside * s, up * s))
        });
        canvas.fill_capsule(root, tip, TAIL_RADII[0] * s, TAIL_RADII[1] * s, DARK, 1.0);
    }

    /// Ухо висит вниз и отлетает вперёд-назад в такт передним лапам.
    fn ear(&self, canvas: &mut Canvas, side: f32) {
        let pose = self.pen.pose;
        let (s, r) = (pose.scale, pose.head.radius);
        let gait = pose
            .fore
            .map_or(0.0, |[a, b]| (a.z - b.z) / (FLOP_LIFT * s));
        let flop = EAR_FLOP * gait.clamp(-1.0, 1.0) * side;
        let root = self.on_head(EAR_ROOT, side);
        let tip = root + Vec3::new(0.0, 0.0, -EAR_LENGTH * r) + self.frame.forward * (flop * r);
        let (a, b) = (self.pen.on_head(root), self.pen.on_head(tip));
        canvas.fill_capsule(a, b, EAR_RADII[0] * s, EAR_RADII[1] * s, DARK, 1.0);
    }

    /// Нос на кончике морды, под ним рот и язык.
    fn nose(&self, canvas: &mut Canvas) {
        let (pose, s) = (self.pen.pose, self.pen.pose.scale);
        self.mouth(canvas);
        let tip = self.point([0.0, MUZZLE_ALONG[1], MUZZLE_UP + 0.15], 1.0);
        canvas.fill_circle(tip, NOSE_RADIUS * s, INK, 1.0);
        if pose.look.snout > 0.0 {
            canvas.fill_circle(tip, NOSE_RADIUS * MUD_NOSE * s, MUD, pose.look.snout);
        }
    }

    fn mouth(&self, canvas: &mut Canvas) {
        let (antic, s) = (&self.pen.pose.antic, self.pen.pose.scale);
        if antic.mouth <= 0.0 && !antic.tongue {
            return;
        }
        let center = self.point([0.0, MOUTH_AT[0], MOUTH_AT[1]], 1.0);
        let open = antic.mouth.clamp(0.0, 1.0);
        let radii = Vec2::new(MOUTH_RADII[0] * s, (MOUTH_RADII[1] + MOUTH_OPEN * open) * s);
        canvas.fill_ellipse(center, radii, 0.0, MOUTH, 1.0);
        if antic.tongue {
            let hang = self.point([0.3, MOUTH_AT[0], MOUTH_AT[1] - 0.45], 1.0);
            let [r0, r1] = TONGUE_RADII.map(|r| r * s);
            canvas.fill_capsule(center, hang, r0, r1, TONGUE, 1.0);
        }
    }

    /// Точка чернилами; закрытый — тёмная чёрточка, крестик и распахнутый — как у гуся.
    fn eye(&self, canvas: &mut Canvas, side: f32) {
        let (pose, s) = (self.pen.pose, self.pen.pose.scale);
        let at = self.point(EYE_AT, side);
        if pose.head.eyes_open < 0.5 {
            let d = Vec2::new(LID_LENGTH * s, 0.0);
            canvas.fill_capsule(at - d, at + d, LID_WIDTH * s, LID_WIDTH * s, DARK, 1.0);
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::creature::test_support::{
        self, HEADINGS, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Pose, RigKind};

    const SIDE: Vec2 = Vec2::new(1.0, 0.0);
    const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
    const AWAY: Vec2 = Vec2::new(0.0, -1.0);

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Dachshund, heading)
    }

    #[test]
    fn draws_inside_the_rect_and_nothing_outside() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            assert!(solid(&pixels, COAT) > 100, "{heading:?}");
            assert!(opaque(&pixels) > 700, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn long_low_body_on_four_short_legs() {
        let dog = pose(SIDE);
        assert!(dog.fore.is_some());
        assert!(dog.body.radii[0] > 2.5 * dog.body.radii[2]);
        let mut tucked = dog.clone();
        tucked.tuck = 1.0;
        let (standing, ..) = render(&dog);
        let (sleeping, ..) = render(&tucked);
        assert!(solid(&standing, PAW) > solid(&sleeping, PAW));
    }

    #[test]
    fn face_shows_from_the_front_and_not_from_behind() {
        let (front, ..) = render(&pose(TOWARD));
        let (back, ..) = render(&pose(AWAY));
        assert!(solid(&front, INK) > 0);
        assert_eq!(solid(&back, INK), 0);
        // Висячие уши видны с любой стороны
        assert!(solid(&front, DARK) > 10 && solid(&back, DARK) > 10);
    }

    #[test]
    fn tail_wags_side_to_side() {
        let mut dog = pose(AWAY);
        let (still, ..) = render(&dog);
        dog.wag = 1.0;
        let (left, ..) = render(&dog);
        dog.wag = -1.0;
        let (right, ..) = render(&dog);
        assert_ne!(left, still);
        assert_ne!(left, right);
    }

    #[test]
    fn mouth_tongue_and_dirty_nose_show_from_the_front() {
        let mut dog = pose(TOWARD);
        let (plain, ..) = render(&dog);
        // Мелкое после мозаики чистым цветом не остаётся: сравниваем кадры
        dog.antic.mouth = 1.0;
        let (open, ..) = render(&dog);
        dog.antic.tongue = true;
        let (tongue, ..) = render(&dog);
        assert_ne!(open, plain);
        assert_ne!(tongue, open);
        assert_eq!(solid(&plain, MUD), 0);
        dog.look.snout = 1.0;
        let (dirty, ..) = render(&dog);
        assert!(solid(&dirty, MUD) > 0);
    }
}
