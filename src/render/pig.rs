//! Лупоглазая свинка-копилка: розовое тело с большой головой одним силуэтом,
//! короткие ноги, уши, рыло, хвостик и два белых глаза навыкате со зрачком,
//! глядящим в камеру. Детали головы ложатся за неё или поверх — по тому, с какой
//! стороны шара они сидят; в каждом слое порядок один: уши, рыло, глаза.

use super::creature::{
    INK, Leg, LegLayer, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer,
    toward_viewer, turned_away,
};
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

const PINK: Color = Color(0xF2_9C_9A);
/// Уши, рыло и хвост.
const ROSE: Color = Color(0xDD_76_7C);
/// Веки светлее тела: закрытый глаз не сливается ни с ним, ни с ушами и рылом.
const LID: Color = Color(0xF9_C4_BC);
const HOOF: Color = Color(0xC4_5F_69);
const NOSTRIL: Color = Color(0x8E_3E_4A);
/// Грязь после лужи: пятна на спине и комья на копытах.
pub(super) const MUD: Color = Color(0x7A_55_34);
/// Пятно: по ходу, вбок, вверх — доли полуосей тела; радиус — design px.
const MUD_SPOTS: [([f32; 3], f32); 4] = [
    ([0.3, 0.25, 0.75], 4.0),
    ([-0.3, -0.35, 0.7], 3.3),
    ([-0.6, 0.3, 0.5], 2.8),
    ([0.0, -0.05, 0.9], 2.4),
];
/// Пятно лежит на спине, сплющено, как всё, на что смотрят сверху.
const MUD_SQUASH: f32 = 0.7;
const MUD_HOOF: f32 = 1.15;

// Размеры — design px, умножаются на `pose.scale`; доли — от радиуса головы.
/// Шея толстая: голова сидит прямо на теле.
const NECK_RADIUS: f32 = 6.5;
/// Нога тянется от копыта к бедру, спрятанному в теле: на бегу она не отрывается.
/// Бедро от центра тела в долях его полуосей: по ходу, вбок и вниз.
const HIP: [f32; 3] = [0.55, 0.5, 0.55];
const LEG_RADII: [f32; 2] = [2.3, 2.9];
const LEG_MAX: f32 = 10.0;
/// Хвост от центра тела в долях его полуосей: назад и вверх, от корня к кончику.
const TAIL: [[f32; 2]; 2] = [[0.9, 0.3], [1.15, 0.75]];
const TAIL_RADIUS: f32 = 1.4;
const SIDES: [f32; 2] = [-1.0, 1.0];
/// Ухо: вбок, вперёд, вверх — корень и кончик. Уши на затылке, глаза на лбу.
const EAR: [[f32; 3]; 2] = [[0.6, -0.45, 0.65], [1.0, -0.7, 1.35]];
const EAR_RADII: [f32; 2] = [2.6, 1.0];
/// Рыло ниже глаз: вперёд (основание и кончик) и вверх.
const SNOUT_ALONG: [f32; 2] = [0.7, 1.08];
const SNOUT_UP: f32 = -0.25;
const SNOUT_RADII: [f32; 2] = [3.1, 2.9];
/// Ноздри от кончика рыла вбок; видны, когда рыло смотрит к зрителю.
const NOSTRIL_SIDE: f32 = 0.19;
const NOSTRIL_RADIUS: f32 = 1.0;
const NOSTRIL_FACING: f32 = 0.25;
/// Глаз: вбок, вперёд, вверх.
const EYE_AT: [f32; 3] = [0.6, 0.55, 0.75];
const EYE_RADIUS: f32 = 3.9;
const PUPIL_RADIUS: f32 = 1.4;
/// Грязь на кончике рыла, в долях его радиуса.
const MUD_SNOUT: f32 = 1.05;
/// Рот: вперёд и вверх от центра головы в долях радиуса; полуоси, design px.
const MOUTH_AT: [f32; 2] = [0.85, -0.6];
const MOUTH_RADII: [f32; 2] = [2.4, 0.9];
const MOUTH_OPEN: f32 = 1.8;
/// Язык: вбок от рта; радиусы от корня к кончику.
const TONGUE_SIDE: f32 = 0.35;
const TONGUE_RADII: [f32; 2] = [1.5, 1.2];
const CROSS_REACH: f32 = 0.65;
const CROSS_WIDTH: f32 = 0.6;
const WIDE_EYE: f32 = 1.3;
const WIDE_PUPIL: f32 = 0.65;
/// Насколько глаза уходят к центру головы, когда она отвернулась от зрителя:
/// со спины они целиком за затылком, а не торчат над ним.
const EYE_SINK: f32 = 0.7;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let pig = Pig {
        pen,
        frame: pen.pose.head_frame(),
    };
    let legs = pen.legs(HIP, LEG_MAX);
    pig.legs(canvas, &legs, LegLayer::Back);
    pig.details(canvas, false);
    pig.pink(canvas);
    pig.legs(canvas, &legs, LegLayer::Middle);
    pig.head(canvas);
    pig.legs(canvas, &legs, LegLayer::Front);
    pig.details(canvas, true);
}

struct Pig<'a> {
    pen: &'a Pen<'a>,
    frame: Frame,
}

impl Pig<'_> {
    /// Смещение от центра головы: доли её радиуса вбок, вперёд и вверх.
    fn on_head(&self, at: [f32; 3], side: f32) -> Vec3 {
        (self.frame.right * (at[0] * side) + self.frame.forward * at[1] + self.frame.up * at[2])
            * self.pen.pose.head.radius
    }

    fn eye_at(&self, side: f32) -> Vec3 {
        let away = (-toward_viewer(self.frame.forward)).clamp(0.0, 1.0);
        self.on_head(EYE_AT, side) * (1.0 - EYE_SINK * away)
    }

    /// Тело и шея; голова тем же цветом — швов нет.
    fn pink(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, PINK, 1.0);
        // Грязь на спине под головой: морда её закрывает, а не наоборот
        self.mud_spots(canvas);
        let radius = NECK_RADIUS * pose.scale;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, PINK, 1.0);
        }
    }

    fn head(&self, canvas: &mut Canvas) {
        let (pen, head) = (self.pen, &self.pen.pose.head);
        canvas.fill_circle(pen.at(head.center), head.radius, PINK, 1.0);
    }

    /// Ноги слоя от дальней к ближней.
    fn legs(&self, canvas: &mut Canvas, legs: &[Leg], layer: LegLayer) {
        let (pen, mud) = (self.pen, self.pen.pose.look.mud);
        for leg in legs.iter().filter(|leg| leg.layer == layer) {
            let (a, b, k) = (pen.at(leg.sole), pen.at(leg.top), leg.size);
            canvas.fill_capsule(a, b, LEG_RADII[0] * k, LEG_RADII[1] * k, HOOF, 1.0);
            if mud > 0.0 {
                canvas.fill_circle(a, LEG_RADII[0] * k * MUD_HOOF, MUD, mud);
            }
        }
    }

    fn mud_spots(&self, canvas: &mut Canvas) {
        let (pen, pose, body) = (self.pen, self.pen.pose, &self.pen.pose.body);
        if pose.look.mud <= 0.0 {
            return;
        }
        let frame = body.frame();
        let (forward, right, up) = (
            frame.forward * body.radii[0],
            frame.right * body.radii[1],
            frame.up * body.radii[2],
        );
        for ([along, side, high], radius) in MUD_SPOTS {
            let at = pen.at(body.center + forward * along + right * side + up * high);
            let r = radius * pose.scale;
            let radii = crate::math::Vec2::new(r, r * MUD_SQUASH);
            canvas.fill_ellipse(at, radii, 0.0, MUD, pose.look.mud);
        }
    }

    /// Детали по одну сторону от розового: `front` — те, что поверх него.
    fn details(&self, canvas: &mut Canvas, front: bool) {
        if self.pen.near_side(self.tail_points()[0]) == front {
            self.tail(canvas);
        }
        // Отвернулась — глаза за головой оба: на затылке глаз не бывает
        let away = turned_away(self.frame.forward);
        let eye_front = |side| !away && faces_viewer(self.eye_at(side));
        for side in SIDES {
            // Ухо не ложится поверх своего глаза: за головой они вместе; со спины глаз нет
            let ear_front = (away || eye_front(side)) && faces_viewer(self.on_head(EAR[0], side));
            if ear_front == front {
                self.ear(canvas, side);
            }
        }
        if faces_viewer(self.frame.forward) == front {
            self.snout(canvas);
        }
        // Глаза последними в слое, дальний раньше ближнего
        let depth = |side| toward_viewer(self.eye_at(side));
        let far_first = if depth(SIDES[0]) <= depth(SIDES[1]) {
            SIDES
        } else {
            [SIDES[1], SIDES[0]]
        };
        for side in far_first {
            if eye_front(side) == front {
                self.eye(canvas, side);
            }
        }
    }

    fn tail_points(&self) -> [Vec3; 2] {
        let body = &self.pen.pose.body;
        let frame = body.frame();
        let back = frame.forward * -body.radii[0];
        let up = frame.up * body.radii[2];
        TAIL.map(|[b, u]| body.center + back * b + up * u)
    }

    fn tail(&self, canvas: &mut Canvas) {
        let pen = self.pen;
        let [root, tip] = self.tail_points().map(|p| pen.at(p));
        let radius = TAIL_RADIUS * pen.pose.scale;
        canvas.fill_capsule(root, tip, radius, radius, ROSE, 1.0);
    }

    fn ear(&self, canvas: &mut Canvas, side: f32) {
        let s = self.pen.pose.scale;
        let [root, tip] = EAR.map(|at| self.pen.on_head(self.on_head(at, side)));
        canvas.fill_capsule(root, tip, EAR_RADII[0] * s, EAR_RADII[1] * s, ROSE, 1.0);
    }

    fn snout(&self, canvas: &mut Canvas) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let point = |along, side| pen.on_head(self.on_head([side, along, SNOUT_UP], 1.0));
        let (base, tip) = (point(SNOUT_ALONG[0], 0.0), point(SNOUT_ALONG[1], 0.0));
        let radii = SNOUT_RADII.map(|r| r * s);
        canvas.fill_capsule(base, tip, radii[0], radii[1], ROSE, 1.0);
        self.mouth(canvas);
        if pen.pose.look.snout > 0.0 {
            canvas.fill_circle(tip, radii[1] * MUD_SNOUT, MUD, pen.pose.look.snout);
        }
        if toward_viewer(self.frame.forward) < NOSTRIL_FACING {
            return;
        }
        for side in SIDES {
            let at = point(SNOUT_ALONG[1], NOSTRIL_SIDE * side);
            canvas.fill_circle(at, NOSTRIL_RADIUS * s, NOSTRIL, 1.0);
        }
    }

    /// Тёмный рот под рылом и язык набок.
    fn mouth(&self, canvas: &mut Canvas) {
        let (pen, antic, s) = (self.pen, &self.pen.pose.antic, self.pen.pose.scale);
        if antic.mouth <= 0.0 && !antic.tongue {
            return;
        }
        let at = |along, up| pen.on_head(self.on_head([0.0, along, up], 1.0));
        let center = at(MOUTH_AT[0], MOUTH_AT[1]);
        let open = antic.mouth.clamp(0.0, 1.0);
        let radii = Vec2::new(MOUTH_RADII[0] * s, (MOUTH_RADII[1] + MOUTH_OPEN * open) * s);
        if open > 0.0 {
            canvas.fill_ellipse(center, radii, 0.0, NOSTRIL, 1.0);
        }
        if antic.tongue {
            let hang =
                pen.on_head(self.on_head([TONGUE_SIDE, MOUTH_AT[0], MOUTH_AT[1] - 0.3], 1.0));
            canvas.fill_capsule(
                center,
                hang,
                TONGUE_RADII[0] * s,
                TONGUE_RADII[1] * s,
                TONGUE,
                1.0,
            );
        }
    }

    /// Белый шар с маленьким зрачком, глядящим в камеру; закрытый глаз — веко
    /// того же размера. Крестик — без белка, распахнутый — крупнее с малым зрачком.
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
    use crate::math::Vec2;
    use crate::render::creature::test_support::{
        self, HEADINGS, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Pose, RigKind};

    const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
    const AWAY: Vec2 = Vec2::new(0.0, -1.0);

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Pig, heading)
    }

    #[test]
    fn draws_inside_the_rect_and_nothing_outside() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            // Спереди тело сжато глубиной и закрыто мордой: чистого розового мало
            assert!(solid(&pixels, PINK) > 100, "{heading:?}");
            assert!(opaque(&pixels) > 900, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn stands_on_four_legs() {
        let pig = pose(Vec2::new(1.0, 0.0));
        assert!(pig.fore.is_some());
        let mut tucked = pig.clone();
        tucked.tuck = 1.0;
        let (standing, ..) = render(&pig);
        let (sleeping, ..) = render(&tucked);
        assert!(solid(&standing, HOOF) > solid(&sleeping, HOOF));
    }

    #[test]
    fn in_profile_legs_grow_from_under_the_belly() {
        use crate::render::creature::test_support::{legs_over_body, one_side_legs};
        for near in [false, true] {
            let pose = one_side_legs(RigKind::Pig, near);
            let (pixels, w, _) = render(&pose);
            let (all, over) = legs_over_body(&pose, &pixels, w, HOOF);
            assert!(all > 20 && over == 0, "{near} {all} {over}");
        }
    }

    #[test]
    fn eyes_bulge_white_and_pupils_show_only_from_the_front() {
        let (front, ..) = render(&pose(TOWARD));
        let (back, ..) = render(&pose(AWAY));
        assert!(solid(&front, WHITE) > 60, "{}", solid(&front, WHITE));
        assert!(solid(&front, INK) > 0);
        assert_eq!(solid(&back, INK), 0);
        assert!(solid(&back, WHITE) < solid(&front, WHITE));
    }

    #[test]
    fn ears_show_over_the_head_from_behind() {
        use crate::render::creature::{rect, test_support::MARGIN};
        use crate::rig::Creature;
        let pig = pose(AWAY);
        let (pixels, w, _) = render(&pig);
        let top = rect(&pig, Creature::extent(test_support::SCALE)).y - MARGIN as i32;
        let head = (pig.screen(pig.head.center).y - top as f32) as usize;
        // Выше центра головы розовое тёмное — только уши
        let ears = pixels[..head * w]
            .iter()
            .filter(|p| **p == 0xFF00_0000 | ROSE.0)
            .count();
        assert!(ears > 20, "{ears}");
    }

    #[test]
    fn mud_shows_on_the_back_from_every_side_and_only_when_muddy() {
        for heading in HEADINGS {
            let mut pig = pose(heading);
            let (clean, ..) = render(&pig);
            assert_eq!(solid(&clean, MUD), 0, "{heading:?}");
            pig.look.mud = 1.0;
            let (muddy, ..) = render(&pig);
            assert!(solid(&muddy, MUD) > 20, "{heading:?}");
        }
    }

    #[test]
    fn crossed_eyes_have_no_whites_and_wide_ones_more() {
        let mut pig = pose(TOWARD);
        let (normal, ..) = render(&pig);
        pig.antic.eyes = Eyes::Wide;
        let (wide, ..) = render(&pig);
        pig.antic.eyes = Eyes::Crossed;
        let (crossed, ..) = render(&pig);
        assert!(solid(&wide, WHITE) > solid(&normal, WHITE));
        assert_eq!(solid(&crossed, WHITE), 0);
        assert!(solid(&crossed, INK) > 0);
    }

    #[test]
    fn mouth_tongue_and_snout_mud_show_from_the_front() {
        let mut pig = pose(TOWARD);
        let (plain, ..) = render(&pig);
        pig.antic.mouth = 1.0;
        let (open, ..) = render(&pig);
        assert!(solid(&open, NOSTRIL) > solid(&plain, NOSTRIL));
        pig.antic.tongue = true;
        let (tongue, ..) = render(&pig);
        assert!(solid(&tongue, TONGUE) > 0);
        assert_eq!(solid(&plain, MUD), 0);
        pig.look.snout = 1.0;
        let (dirty, ..) = render(&pig);
        assert!(solid(&dirty, MUD) > 0);
    }

    #[test]
    fn closed_eyes_turn_into_lids() {
        let mut pig = pose(TOWARD);
        let (open, ..) = render(&pig);
        pig.head.eyes_open = 0.0;
        let (closed, ..) = render(&pig);
        assert_eq!(solid(&closed, WHITE), 0);
        assert_eq!(solid(&open, LID), 0);
        assert!(solid(&closed, LID) > 60);
    }

    #[test]
    fn eyes_stay_whole_above_the_snout_from_every_side() {
        use crate::render::creature::test_support::{AT, SCALE};
        use crate::rig::{Creature, Gesture, RigInput};
        use crate::rng::Rng;
        let visible_eye = std::f32::consts::PI * (EYE_RADIUS * SCALE).powi(2) * 0.45;
        for step in 0..16 {
            let heading = Vec2::from_angle(step as f32 * std::f32::consts::TAU / 16.0);
            let mut pig = Creature::new(RigKind::Pig, AT, heading, SCALE);
            let input = RigInput {
                pos: AT,
                vel: Vec2::ZERO,
                heading,
                look_at: None,
                gesture: Gesture::Look,
                progress: 0.25,
                scale: SCALE,
                grounded: true,
                antic: crate::rig::Antic::default(),
            };
            for _ in 0..90 {
                pig.update(1.0 / 60.0, &input, &mut Rng::new(7), &mut Vec::new());
            }
            let pose = pig.pose();
            let frame = pose.head_frame();
            if toward_viewer(frame.forward) < 0.0 {
                continue;
            }
            // Лицом к зрителю ни ухо, ни рыло не съедают глаз: белого не меньше одного шара
            let (pixels, ..) = render(pose);
            assert!(solid(&pixels, WHITE) as f32 > visible_eye, "{heading:?}");
            let pen = Pen {
                origin: Vec2::ZERO,
                pose,
            };
            let pig = Pig { pen: &pen, frame };
            let snout = pen.on_head(pig.on_head([0.0, SNOUT_ALONG[1], SNOUT_UP], 1.0));
            for side in SIDES {
                let eye = pen.on_head(pig.eye_at(side));
                assert!(eye.y < snout.y, "{heading:?}");
            }
        }
    }
}
