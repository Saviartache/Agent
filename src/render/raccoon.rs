//! Енот: серое тело с головой одним силуэтом, тёмные лапы, ушки треугольником,
//! острая светлая мордочка с чёрным носом, тёмная маска с белыми глазами и пушистый
//! хвост кольцами, кончик которого отстаёт на пружине. Детали головы ложатся за неё
//! или поверх по той же стороне шара, что у свинки: уши, маска, мордочка, глаза.

use super::creature::{
    INK, Leg, LegLayer, MOUTH, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer,
    toward_viewer, turned_away,
};
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Eyes, Frame, NECK_POINTS, Vec3};

/// Серый с тёплым отливом: на сером рабочем столе не теряется.
const GRAY: Color = Color(0xA8_9F_94);
/// Мордочка, брови и светлые кольца хвоста.
const LIGHT: Color = Color(0xE6_E0_D6);
/// Маска и тёмные кольца хвоста.
const MASK: Color = Color(0x3A_35_31);
const EAR: Color = Color(0x7D_74_6A);
const PAW: Color = Color(0x4E_48_42);

// Размеры — design px, умножаются на `pose.scale`; доли — от радиуса головы.
const NECK_RADIUS: f32 = 4.5;
/// Бедро от центра тела в долях его полуосей: по ходу, вбок и вниз.
const HIP: [f32; 3] = [0.6, 0.45, 0.5];
const LEG_RADII: [f32; 2] = [1.8, 2.4];
const LEG_MAX: f32 = 10.0;
/// Хвост от центра тела в долях полуосей (назад, вверх): корень, изгиб, кончик.
const TAIL: [[f32; 2]; 3] = [[0.8, 0.35], [1.55, 0.7], [2.2, 0.45]];
/// Толщина хвоста от корня через середину к кончику.
const TAIL_RADII: [f32; 3] = [2.4, 3.4, 2.6];
/// Колец, считая тёмный кончик.
const TAIL_RINGS: usize = 6;
const SIDES: [f32; 2] = [-1.0, 1.0];
/// Ухо: вбок, вперёд, вверх — корень и кончик.
const EAR_AT: [[f32; 3]; 2] = [[0.55, -0.3, 0.7], [0.8, -0.35, 1.45]];
const EAR_RADII: [f32; 2] = [2.2, 0.5];
/// Мордочка ниже глаз: вперёд (основание и кончик) и вверх.
const SNOUT_ALONG: [f32; 2] = [0.55, 1.5];
const SNOUT_UP: f32 = -0.2;
const SNOUT_RADII: [f32; 2] = [3.0, 1.1];
const NOSE_RADIUS: f32 = 1.4;
/// Светлое лицо: вбок, вперёд, вверх и радиус.
const FACE_AT: [f32; 3] = [0.0, 0.4, 0.05];
const FACE_RADIUS: f32 = 0.72;
/// Маска тянется от щеки через глаз к переносице.
const MASK_CHEEK: [f32; 3] = [0.8, 0.3, 0.2];
const MASK_BRIDGE: [f32; 3] = [0.0, 0.85, 0.3];
const MASK_RADII: [f32; 2] = [2.2, 3.0];
/// Глаз: вбок, вперёд, вверх.
const EYE_AT: [f32; 3] = [0.5, 0.68, 0.3];
const EYE_RADIUS: f32 = 1.7;
const PUPIL_RADIUS: f32 = 0.9;
const LID_WIDTH: f32 = 0.6;
/// Рот: вперёд и вверх от центра головы в долях радиуса; полуоси, design px.
const MOUTH_AT: [f32; 2] = [0.95, -0.5];
const MOUTH_RADII: [f32; 2] = [1.8, 0.7];
const MOUTH_OPEN: f32 = 1.4;
const TONGUE_SIDE: f32 = 0.3;
const TONGUE_RADII: [f32; 2] = [1.2, 1.0];
const CROSS_REACH: f32 = 0.9;
const CROSS_WIDTH: f32 = 0.5;
const WIDE_EYE: f32 = 1.35;
const WIDE_PUPIL: f32 = 0.6;
/// Глаза уходят к центру головы, когда она отвернулась от зрителя.
const EYE_SINK: f32 = 0.7;

pub(super) fn draw(canvas: &mut Canvas, pen: &Pen) {
    let raccoon = Raccoon {
        pen,
        frame: pen.pose.head_frame(),
    };
    let legs = pen.legs(HIP, LEG_MAX);
    raccoon.legs(canvas, &legs, LegLayer::Back);
    raccoon.details(canvas, false);
    raccoon.gray(canvas);
    raccoon.legs(canvas, &legs, LegLayer::Middle);
    raccoon.head(canvas);
    raccoon.legs(canvas, &legs, LegLayer::Front);
    raccoon.details(canvas, true);
}

struct Raccoon<'a> {
    pen: &'a Pen<'a>,
    frame: Frame,
}

impl Raccoon<'_> {
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
    fn gray(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, GRAY, 1.0);
        let radius = NECK_RADIUS * pose.scale;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, GRAY, 1.0);
        }
    }

    fn head(&self, canvas: &mut Canvas) {
        let (pen, head) = (self.pen, &self.pen.pose.head);
        canvas.fill_circle(pen.at(head.center), head.radius, GRAY, 1.0);
    }

    /// Лапы слоя от дальней к ближней.
    fn legs(&self, canvas: &mut Canvas, legs: &[Leg], layer: LegLayer) {
        for leg in legs.iter().filter(|leg| leg.layer == layer) {
            let (a, b, k) = (self.pen.at(leg.sole), self.pen.at(leg.top), leg.size);
            canvas.fill_capsule(a, b, LEG_RADII[0] * k, LEG_RADII[1] * k, PAW, 1.0);
        }
    }

    /// Детали по одну сторону от серого: `front` — те, что поверх него.
    fn details(&self, canvas: &mut Canvas, front: bool) {
        if self.pen.near_side(self.on_back(TAIL[0])) == front {
            self.tail(canvas);
        }
        let away = turned_away(self.frame.forward);
        let eye_front = |side| !away && faces_viewer(self.eye_at(side));
        for side in SIDES {
            let ear_front =
                (away || eye_front(side)) && faces_viewer(self.on_head(EAR_AT[0], side));
            if ear_front == front {
                self.ear(canvas, side);
            }
        }
        let facing = faces_viewer(self.frame.forward) == front;
        if facing {
            self.face(canvas);
        }
        let depth = |side| toward_viewer(self.eye_at(side));
        let far_first = if depth(SIDES[0]) <= depth(SIDES[1]) {
            SIDES
        } else {
            [SIDES[1], SIDES[0]]
        };
        for side in far_first {
            if eye_front(side) == front {
                self.mask(canvas, side);
            }
        }
        if facing {
            self.snout(canvas);
        }
        for side in far_first {
            if eye_front(side) == front {
                self.eye(canvas, side);
            }
        }
    }

    /// Точка у тела: назад и вверх от центра в долях полуосей.
    fn on_back(&self, [back, up]: [f32; 2]) -> Vec3 {
        let body = &self.pen.pose.body;
        let frame = body.frame();
        body.center + frame.forward * (-body.radii[0] * back) + frame.up * (body.radii[2] * up)
    }

    /// Кольца от корня к кончику; кончик тёмный и отстаёт на пружине.
    fn tail(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let [root, bend, tip] = TAIL.map(|at| self.on_back(at));
        let tip = tip + Vec3::on_ground(pose.tail, 0.0);
        // Сидя столбиком, хвост лежит на земле, а не уходит под неё
        let [root, bend, tip] = [root, bend, tip].map(|p| Vec3::new(p.x, p.y, p.z.max(0.0)));
        let at = |t: f32| {
            let point = root.lerp(bend, t).lerp(bend.lerp(tip, t), t);
            pen.at(point)
        };
        let radius = |t: f32| {
            let [a, b, c] = TAIL_RADII;
            let r = if t < 0.5 {
                a + (b - a) * t * 2.0
            } else {
                b + (c - b) * (t - 0.5) * 2.0
            };
            r * pose.scale
        };
        for ring in 0..TAIL_RINGS {
            let (t0, t1) = (
                ring as f32 / TAIL_RINGS as f32,
                (ring + 1) as f32 / TAIL_RINGS as f32,
            );
            // Тёмные кольца через одно, считая от кончика
            let color = if (TAIL_RINGS - ring) % 2 == 1 {
                MASK
            } else {
                LIGHT
            };
            canvas.fill_capsule(at(t0), at(t1), radius(t0), radius(t1), color, 1.0);
        }
    }

    fn ear(&self, canvas: &mut Canvas, side: f32) {
        let s = self.pen.pose.scale;
        let [root, tip] = EAR_AT.map(|at| self.pen.on_head(self.on_head(at, side)));
        canvas.fill_capsule(root, tip, EAR_RADII[0] * s, EAR_RADII[1] * s, EAR, 1.0);
    }

    /// Светлое лицо спереди головы: на нём читается маска.
    fn face(&self, canvas: &mut Canvas) {
        let pen = self.pen;
        let at = pen.on_head(self.on_head(FACE_AT, 1.0));
        canvas.fill_circle(at, pen.pose.head.radius * FACE_RADIUS, LIGHT, 1.0);
    }

    /// Тёмная полоса от щеки через глаз к переносице.
    fn mask(&self, canvas: &mut Canvas, side: f32) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let cheek = pen.on_head(self.on_head(MASK_CHEEK, side));
        let eye = pen.on_head(self.eye_at(side));
        let bridge = pen.on_head(self.on_head(MASK_BRIDGE, side));
        let [thin, thick] = MASK_RADII.map(|r| r * s);
        canvas.fill_capsule(cheek, eye, thin, thick, MASK, 1.0);
        canvas.fill_capsule(eye, bridge, thick, thin, MASK, 1.0);
    }

    fn snout(&self, canvas: &mut Canvas) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let point = |along| pen.on_head(self.on_head([0.0, along, SNOUT_UP], 1.0));
        let (base, tip) = (point(SNOUT_ALONG[0]), point(SNOUT_ALONG[1]));
        let radii = SNOUT_RADII.map(|r| r * s);
        canvas.fill_capsule(base, tip, radii[0], radii[1], LIGHT, 1.0);
        self.mouth(canvas);
        canvas.fill_circle(tip, NOSE_RADIUS * s, INK, 1.0);
    }

    /// Тёмный рот под мордочкой и язык набок.
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

    /// Белый глаз на маске; закрытый — светлая черточка, крестик — по белому.
    fn eye(&self, canvas: &mut Canvas, side: f32) {
        let (pen, s) = (self.pen, self.pen.pose.scale);
        let center = pen.on_head(self.eye_at(side));
        let mut radius = EYE_RADIUS * s;
        if pen.pose.head.eyes_open < 0.5 {
            let d = Vec2::new(radius, 0.0);
            canvas.fill_capsule(
                center - d,
                center + d,
                LID_WIDTH * s,
                LID_WIDTH * s,
                LIGHT,
                1.0,
            );
            return;
        }
        let eyes = pen.pose.antic.eyes;
        let mut pupil = PUPIL_RADIUS * s;
        if eyes == Eyes::Wide {
            radius *= WIDE_EYE;
            pupil *= WIDE_PUPIL;
        }
        canvas.fill_circle(center, radius + OUTLINE_WIDTH * s, OUTLINE, 1.0);
        canvas.fill_circle(center, radius, WHITE, 1.0);
        if eyes == Eyes::Crossed {
            cross(canvas, center, radius * CROSS_REACH, CROSS_WIDTH * s);
        } else {
            canvas.fill_circle(center, pupil, INK, 1.0);
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

    const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
    const AWAY: Vec2 = Vec2::new(0.0, -1.0);
    const SIDE: Vec2 = Vec2::new(1.0, 0.0);

    fn pose(heading: Vec2) -> Pose {
        test_support::pose(RigKind::Raccoon, heading)
    }

    /// Без мозаики: мелкие детали остаются своего цвета.
    fn flat(pose: &Pose) -> Vec<u32> {
        use crate::render::creature::{paint, rect};
        use crate::rig::Creature;
        let rect = rect(pose, Creature::extent(test_support::SCALE));
        let (w, h) = (rect.width as usize, rect.height as usize);
        let mut pixels = vec![0u32; w * h];
        let mut canvas = Canvas::new(&mut pixels, w, w as u32, h as u32).unwrap();
        paint(&mut canvas, Vec2::new(rect.x as f32, rect.y as f32), pose);
        pixels
    }

    /// Белки глаз: белок тонким кольцом вокруг зрачка, ровно белых точек мало.
    fn whites(pixels: &[u32]) -> usize {
        let bright = |p: u32| (0..4).all(|i| (p >> (8 * i)) & 0xFF >= 0xF0);
        pixels.iter().filter(|p| bright(**p)).count()
    }

    #[test]
    fn draws_inside_the_rect_and_nothing_outside() {
        for heading in HEADINGS {
            let (pixels, w, h) = render(&pose(heading));
            assert!(solid(&flat(&pose(heading)), GRAY) > 100, "{heading:?}");
            assert!(opaque(&pixels) > 900, "{heading:?}");
            assert_eq!(outside_rect(&pixels, w, h), 0, "{heading:?}");
        }
    }

    #[test]
    fn stands_on_four_paws() {
        let raccoon = pose(SIDE);
        assert!(raccoon.fore.is_some());
        let mut tucked = raccoon.clone();
        tucked.tuck = 1.0;
        assert!(solid(&flat(&raccoon), PAW) > solid(&flat(&tucked), PAW));
    }

    #[test]
    fn in_profile_far_legs_show_below_the_belly_and_near_ones_over_it() {
        use crate::render::creature::test_support::{legs_over_body, one_side_legs};
        let far = one_side_legs(RigKind::Raccoon, false);
        let (pixels, w, _) = render(&far);
        let (all, over) = legs_over_body(&far, &pixels, w, PAW);
        assert!(all > 20 && over == 0, "{all} {over}");
        let near = one_side_legs(RigKind::Raccoon, true);
        let (pixels, w, _) = render(&near);
        let (all, over) = legs_over_body(&near, &pixels, w, PAW);
        assert!(all > 20 && over > 0, "{all} {over}");
    }

    #[test]
    fn mask_and_eyes_show_from_the_front_only() {
        let (front, back) = (flat(&pose(TOWARD)), flat(&pose(AWAY)));
        assert!(whites(&front) > 4, "{}", whites(&front));
        assert!(solid(&front, INK) > 0);
        assert!(solid(&front, MASK) > solid(&back, MASK));
        assert_eq!(whites(&back), 0);
    }

    #[test]
    fn tail_is_ringed_and_its_tip_swings_on_the_spring() {
        let still = pose(SIDE);
        let pixels = flat(&still);
        assert!(solid(&pixels, LIGHT) > 20 && solid(&pixels, MASK) > 20);
        let mut swung = still.clone();
        swung.tail = Vec2::new(0.0, 6.0);
        assert_ne!(flat(&swung), pixels);
    }

    #[test]
    fn closed_eyes_turn_into_lids_and_wide_ones_grow() {
        let mut raccoon = pose(TOWARD);
        let normal = whites(&flat(&raccoon));
        raccoon.head.eyes_open = 0.0;
        assert_eq!(whites(&flat(&raccoon)), 0);
        raccoon.head.eyes_open = 1.0;
        raccoon.antic.eyes = Eyes::Wide;
        assert!(whites(&flat(&raccoon)) > normal);
    }

    #[test]
    fn mouth_and_tongue_show_from_the_front() {
        let mut raccoon = pose(TOWARD);
        let plain = flat(&raccoon);
        raccoon.antic.mouth = 1.0;
        raccoon.antic.tongue = true;
        let open = flat(&raccoon);
        assert!(solid(&open, MOUTH) > solid(&plain, MOUTH));
        assert!(solid(&open, TONGUE) > 0);
    }
}
