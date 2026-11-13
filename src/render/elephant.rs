//! Слонёнок: серо-голубое пухлое тело с головой одним силуэтом, ноги-тумбы,
//! уши-лопухи, гибкий хобот и два глаза со зрачком в камеру. Детали головы ложатся
//! за неё или поверх и идут по глубине к зрителю.

mod ears;

use super::creature::{
    INK, Leg, LegLayer, MOUTH, OUTLINE, OUTLINE_WIDTH, Pen, TONGUE, WHITE, cross, faces_viewer,
    toward_viewer, turned_away,
};
use super::{Canvas, Color};
use crate::math::Vec2;
use crate::rig::{Ear, Eyes, Frame, NECK_POINTS, Vec3};

const HIDE: Color = Color(0x93_A8_C2);
/// Уши и хвост.
const EAR: Color = Color(0x7F_95_B1);
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
/// Ногти внизу носка: вбок, вперёд и вниз в долях радиуса ступни.
const TOENAILS: [[f32; 3]; 2] = [[-0.45, 0.6, 0.3], [0.45, 0.6, 0.3]];
const TOENAIL_RADIUS: f32 = 1.3;
/// Хвост от центра тела в долях его полуосей: назад и вверх, от корня к кончику.
const TAIL: [[f32; 2]; 2] = [[0.95, 0.35], [1.2, -0.25]];
const TAIL_RADIUS: f32 = 1.1;
const TUFT_RADIUS: f32 = 1.7;
const SIDES: [f32; 2] = [-1.0, 1.0];
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
    let legs = pen.legs(HIP, LEG_MAX);
    elephant.legs(canvas, &legs, LegLayer::Back);
    elephant.details(canvas, Layer::Back);
    elephant.body(canvas);
    elephant.details(canvas, Layer::Middle);
    elephant.legs(canvas, &legs, LegLayer::Middle);
    elephant.head(canvas);
    elephant.legs(canvas, &legs, LegLayer::Front);
    elephant.details(canvas, Layer::Front);
}

struct Elephant<'a> {
    pen: &'a Pen<'a>,
    frame: Frame,
}

/// Слой детали: за телом, между телом и головой, поверх всего серого.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Layer {
    Back,
    Middle,
    Front,
}

/// Деталь головы: её слой и насколько она ближе центра головы к зрителю, px.
#[derive(Clone, Copy)]
struct Part {
    layer: Layer,
    depth: f32,
    kind: PartKind,
}

#[derive(Clone, Copy)]
enum PartKind {
    Ear(usize),
    /// Хобот и рот.
    Trunk,
    Eyes,
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

    /// Тело и шея; голова тем же цветом — швов нет.
    fn body(&self, canvas: &mut Canvas) {
        let (pen, pose) = (self.pen, self.pen.pose);
        let body = pen.body();
        canvas.fill_ellipse(body.center, body.radii, body.rotation, HIDE, 1.0);
        let radius = NECK_RADIUS * pose.scale;
        for i in 1..NECK_POINTS {
            let (a, b) = (pen.at(pose.neck[i - 1]), pen.at(pose.neck[i]));
            canvas.fill_capsule(a, b, radius, radius, HIDE, 1.0);
        }
    }

    fn head(&self, canvas: &mut Canvas) {
        let (pen, head) = (self.pen, &self.pen.pose.head);
        canvas.fill_circle(pen.at(head.center), head.radius, HIDE, 1.0);
    }

    /// Ноги слоя от дальней к ближней; ногти — на носке стоящей ноги, со стороны зрителя.
    fn legs(&self, canvas: &mut Canvas, legs: &[Leg], layer: LegLayer) {
        let (pen, pose) = (self.pen, self.pen.pose);
        // На боку и поджатой ногой ногтей не видно
        let upright = pose.body.frame().up.z > 0.7 && pose.tuck <= 0.0;
        let ahead = Vec3::on_ground(pose.body.forward, 0.0);
        let side = Vec3::new(-ahead.y, ahead.x, 0.0);
        let nails =
            TOENAILS.map(|[x, along, down]| side * x + ahead * along + Vec3::new(0.0, 0.0, -down));
        for leg in legs.iter().filter(|leg| leg.layer == layer) {
            let (a, k) = (pen.at(leg.sole), leg.size);
            let radius = LEG_RADII[0] * k;
            canvas.fill_capsule(a, pen.at(leg.top), radius, LEG_RADII[1] * k, LEG, 1.0);
            for nail in nails.into_iter().filter(|n| upright && faces_viewer(*n)) {
                let at = pen.at(leg.sole + nail * radius);
                canvas.fill_circle(at, TOENAIL_RADIUS * k, TOENAIL, 1.0);
            }
        }
    }

    /// Детали слоя от дальней к ближней.
    fn details(&self, canvas: &mut Canvas, layer: Layer) {
        let tail_front = self.pen.near_side(self.tail_points()[0]);
        if layer == [Layer::Back, Layer::Front][usize::from(tail_front)] {
            self.tail(canvas);
        }
        let mut parts = self.head_parts();
        parts.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        for part in parts.into_iter().filter(|part| part.layer == layer) {
            match part.kind {
                PartKind::Ear(i) => {
                    if let Some(ears) = &self.pen.pose.ears {
                        ears::draw(canvas, self.pen, &ears[i]);
                    }
                }
                PartKind::Trunk => {
                    self.mouth(canvas);
                    self.trunk(canvas);
                }
                PartKind::Eyes => {
                    for side in SIDES.into_iter().filter(|side| self.eye_front(*side)) {
                        self.eye(canvas, side);
                    }
                }
            }
        }
    }

    /// Ухо за головой, когда его шарнир дальше её центра, а за телом — когда тело его
    /// закрывает; ближнее ложится поверх головы, но под морду, если она ближе. Морда —
    /// хобот, рот и глаза одним слоем.
    fn head_parts(&self) -> [Part; 4] {
        let pose = self.pen.pose;
        let depth = |at: Vec3| toward_viewer(at - pose.head.center);
        let face = toward_viewer(self.on_head([0.0, EYE_AT[1], EYE_AT[2]], 1.0));
        let ear = |i: usize| {
            let ear = pose.ears.map_or_else(Ear::default, |ears| ears[i]);
            let root = ear.root;
            let layer = if depth(root) >= 0.0 {
                Layer::Front
            } else if self.pen.behind_body(ear.center) {
                Layer::Back
            } else {
                Layer::Middle
            };
            Part {
                layer,
                depth: depth(root),
                kind: PartKind::Ear(i),
            }
        };
        // Хобот за головой, только когда морда отвернулась; кивок вниз его не прячет
        let ahead = !turned_away(Vec3::on_ground(pose.head.dir, 0.0));
        let trunk = match pose.trunk {
            Some([root, _]) if !ahead => depth(root),
            _ => face,
        };
        [
            ear(0),
            ear(1),
            Part {
                layer: if ahead { Layer::Front } else { Layer::Back },
                depth: trunk,
                kind: PartKind::Trunk,
            },
            // Глаз за головой не рисуется: из-за широкого лба он торчал бы бровью
            Part {
                layer: Layer::Front,
                depth: face,
                kind: PartKind::Eyes,
            },
        ]
    }

    fn eye_front(&self, side: f32) -> bool {
        !turned_away(self.frame.forward) && faces_viewer(self.eye_at(side))
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
        let s = pen.pose.scale;
        canvas.fill_capsule(root, tip, TAIL_RADIUS * s, TAIL_RADIUS * s, EAR, 1.0);
        canvas.fill_circle(tip, TUFT_RADIUS * s, TUFT, 1.0);
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
        self, HEADINGS, MARGIN, SCALE, opaque, outside_rect, render, solid,
    };
    use crate::rig::{Antic, Creature, Gesture, Pose, RigInput, RigKind};
    use crate::rng::Rng;

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
    fn in_profile_far_legs_show_below_the_belly_and_near_ones_over_it() {
        use crate::render::creature::test_support::{legs_over_body, one_side_legs};
        let far = one_side_legs(RigKind::Elephant, false);
        let (pixels, w, _) = render(&far);
        let (all, over) = legs_over_body(&far, &pixels, w, LEG);
        assert!(all > 20 && over == 0, "{all} {over}");
        let near = one_side_legs(RigKind::Elephant, true);
        let (pixels, w, _) = render(&near);
        let (all, over) = legs_over_body(&near, &pixels, w, LEG);
        assert!(all > 20 && over > 0, "{all} {over}");
    }

    #[test]
    fn toenails_are_on_the_toes_and_hidden_from_behind() {
        let (back, ..) = render(&pose(AWAY));
        assert!(solid(&back, LEG) > 0);
        assert_eq!(near(&back, TOENAIL), 0);
        let (front, ..) = render(&pose(TOWARD));
        assert!(near(&front, TOENAIL) > 0);
    }

    #[test]
    fn ear_inner_and_rim_narrow_together_when_turning() {
        let mut last: Option<[f32; 2]> = None;
        for step in 0..=60 {
            // От анфаса через профиль к спине
            let angle = std::f32::consts::FRAC_PI_2 - step as f32 * std::f32::consts::PI / 60.0;
            let elephant = posed(Vec2::from_angle(angle), Gesture::Idle, Antic::default());
            let pen = Pen {
                origin: Vec2::ZERO,
                pose: &elephant,
            };
            let ears = elephant.ears.unwrap();
            let share = ears.map(|ear| ears::inner_share(&pen, &ear));
            for (ear, share) in ears.iter().zip(share) {
                // Повёрнутое к зрителю ухо — розовое с каймой, а не серое с розовой щелью
                if toward_viewer(ear.inner) > 0.5 {
                    assert!(share > 0.2, "{step} {share}");
                }
                if toward_viewer(ear.inner) < 0.1 {
                    assert_eq!(share, 0.0, "{step}");
                }
            }
            if let Some(last) = last {
                for (a, b) in last.into_iter().zip(share) {
                    assert!((a - b).abs() < 0.15, "{step} {a} {b}");
                }
            }
            last = Some(share);
        }
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
        let flapping = Antic {
            wings: 1.0,
            flap: 0.25,
            ..Antic::default()
        };
        // Ухо далеко за контуром головы: наружная сторона и изнанка
        let wide = |antic| {
            let pose = posed(TOWARD, Gesture::Idle, antic);
            let [a, b] = beside(&pose, EAR, 1.5);
            let [c, d] = beside(&pose, ears::INNER, 1.5);
            a + b + c + d
        };
        let (calm, spread) = (wide(Antic::default()), wide(flapping));
        assert!(spread > calm + 30, "{calm} {spread}");
    }

    /// Слои ушей, левого и правого.
    fn ear_layers(pose: &Pose) -> [Layer; 2] {
        let pen = Pen {
            origin: Vec2::ZERO,
            pose,
        };
        let elephant = Elephant {
            pen: &pen,
            frame: pose.head_frame(),
        };
        let parts = elephant.head_parts();
        [parts[0].layer, parts[1].layer]
    }

    #[test]
    fn ears_go_behind_the_head_from_the_front_and_over_it_from_behind() {
        assert!(ear_layers(&pose(TOWARD)).iter().all(|l| *l != Layer::Front));
        assert_eq!(ear_layers(&pose(AWAY)), [Layer::Front; 2]);
        // В профиль ближнее ухо — правое по ходу вправо
        let [far, near] = ear_layers(&pose(RIGHT));
        assert_eq!(near, Layer::Front);
        assert_ne!(far, Layer::Front);
        let [near, far] = ear_layers(&pose(Vec2::new(-1.0, 0.0)));
        assert_eq!(near, Layer::Front);
        assert_ne!(far, Layer::Front);
        // Вполоборота ближнее ухо поверх головы, но под мордой
        let elephant = pose(Vec2::new(0.7, 0.7));
        let pen = Pen {
            origin: Vec2::ZERO,
            pose: &elephant,
        };
        let parts = Elephant {
            pen: &pen,
            frame: elephant.head_frame(),
        }
        .head_parts();
        let (near, eyes) = (parts[1], parts[3]);
        assert_eq!(near.layer, Layer::Front);
        assert!(near.depth < eyes.depth);
    }

    /// Пиксели цвета левее и правее головы дальше `reach` её радиусов от центра.
    fn beside(pose: &Pose, color: Color, reach: f32) -> [usize; 2] {
        let (pixels, w, _) = render(pose);
        let rect = super::super::creature::rect(pose, Creature::extent(SCALE));
        let origin = Vec2::new(rect.x as f32, rect.y as f32) - Vec2::new(1.0, 1.0) * MARGIN as f32;
        let center = pose.screen(pose.head.center) - origin;
        let gap = pose.head.radius * reach;
        [-1.0, 1.0].map(|out| {
            let mut side = pixels.clone();
            for (i, p) in side.iter_mut().enumerate() {
                if ((i % w) as f32 - center.x) * out <= gap {
                    *p = 0;
                }
            }
            near(&side, color)
        })
    }

    #[test]
    fn from_the_front_both_ears_stick_out_pink_inside() {
        let elephant = pose(TOWARD);
        let out = beside(&elephant, EAR, 1.0);
        assert!(out.iter().all(|n| *n > 30), "{out:?}");
        let inner = beside(&elephant, ears::INNER, 0.0);
        assert!(inner.iter().all(|n| *n > 20), "{inner:?}");
    }

    #[test]
    fn from_behind_and_in_profile_ears_show_the_grey_outside() {
        for heading in [AWAY, RIGHT, Vec2::new(-1.0, 0.0), Vec2::new(0.7, -0.7)] {
            let (pixels, ..) = render(&pose(heading));
            assert_eq!(near(&pixels, ears::INNER), 0, "{heading:?}");
            assert!(near(&pixels, EAR) > 50, "{heading:?}");
        }
        let out = beside(&pose(AWAY), EAR, 1.0);
        assert!(out.iter().all(|n| *n > 20), "{out:?}");
    }

    #[test]
    fn asleep_ears_stay_below_the_crown() {
        for (x, y) in [(1.0, 0.0), (0.7, 0.7), (0.0, 1.0), (-0.7, 0.7)] {
            for heading in [Vec2::new(x, y), Vec2::new(-x, -y)] {
                let elephant = posed(heading, Gesture::Sleep, Antic::default());
                let pen = Pen {
                    origin: Vec2::ZERO,
                    pose: &elephant,
                };
                let crown = pen.at(elephant.head.center).y - elephant.head.radius;
                for ear in elephant.ears.unwrap() {
                    let top = ears::top(&pen, &ear);
                    assert!(top >= crown, "{heading:?} {top} {crown}");
                }
            }
        }
    }

    /// Слоник секунду в жесте и накладке, взгляд прямо по ходу.
    fn posed(heading: Vec2, gesture: Gesture, antic: Antic) -> Pose {
        let at = test_support::AT;
        let mut rig = Creature::new(RigKind::Elephant, at, heading, SCALE);
        let input = RigInput {
            pos: at,
            vel: Vec2::ZERO,
            heading,
            look_at: Some(at + heading * 1000.0),
            gesture,
            progress: 0.0,
            scale: SCALE,
            grounded: true,
            antic,
        };
        let mut rng = Rng::new(7);
        for _ in 0..60 {
            rig.update(1.0 / 60.0, &input, &mut rng, &mut Vec::new());
        }
        rig.pose().clone()
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
