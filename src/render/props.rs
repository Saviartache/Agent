//! Предметы сценок на земле: тень шашечкой и плоские фигуры в общем стиле.
//! Глубина сжата вдвое, объём — только высотой; обводка у белого и у курсора.

use std::f32::consts::TAU;

use super::shadow::{self, Oval};
use super::{Canvas, Color};
use crate::effects::props::{Prop, PropKind, Props};
use crate::math::{Rect, Vec2};

const INK: Color = Color(0x1B_1F_29);
const WHITE: Color = Color(0xFF_FF_FF);
const WHITE_EDGE: Color = Color(0xB9_C0_CB);
const BROWN: [Color; 3] = [Color(0x6B_44_23), Color(0x7E_55_31), Color(0x8F_65_3A)];
const FLESH: Color = Color(0xF2_E3_B3);
const YELLOW: Color = Color(0xF2_C9_4C);
/// Сжатие глубины на земле.
const DEPTH: f32 = 0.5;
/// Тень слабеет вдвое на такой высоте, доли радиуса.
const SHADOW_HALF: f32 = 2.0;
/// Самая узкая монетка ребром, доля диаметра.
const COIN_EDGE: f32 = 0.18;

/// Тени — до всего остального на холсте.
pub(super) fn shadows(canvas: &mut Canvas, origin: Vec2, props: &Props, cell: u32) {
    for prop in props.iter().filter(|prop| prop.kind != PropKind::Butterfly) {
        let width = match prop.kind {
            PropKind::Coin => coin_width(prop),
            _ => 1.0,
        };
        let spot = Oval {
            center: prop.pos,
            radii: Vec2::new(
                prop.radius * 0.95 * width,
                prop.radius * 0.95 * DEPTH * width,
            ),
            rotation: 0.0,
        };
        let height = 1.0 / (1.0 + prop.z / (prop.radius * SHADOW_HALF).max(1.0));
        let opacity = shadow::OPACITY * prop.opacity() * height;
        shadow::dotted(canvas, origin, &spot, cell, opacity);
    }
}

pub(super) fn draw(canvas: &mut Canvas, origin: Vec2, props: &Props) {
    for prop in props.iter() {
        draw_one(canvas, origin, prop);
    }
}

/// Монетка вращается вокруг вертикальной оси: ширина по `cos`.
fn coin_width(prop: &Prop) -> f32 {
    prop.angle.cos().abs().max(COIN_EDGE)
}

/// Плоская заливка с общим набором фигур вида.
struct Pen<'a, 'b> {
    canvas: &'a mut Canvas<'b>,
    opacity: f32,
}

impl Pen<'_, '_> {
    fn circle(&mut self, at: Vec2, radius: f32, color: Color) {
        self.canvas.fill_circle(at, radius, color, self.opacity);
    }

    fn oval(&mut self, at: Vec2, radii: Vec2, rotation: f32, color: Color) {
        self.canvas
            .fill_ellipse(at, radii, rotation, color, self.opacity);
    }

    fn line(&mut self, a: Vec2, b: Vec2, ra: f32, rb: f32, color: Color) {
        self.canvas.fill_capsule(a, b, ra, rb, color, self.opacity);
    }

    /// Треугольник веером капсул: `grow` — на сколько он разбухает.
    fn triangle(&mut self, [a, b, c]: [Vec2; 3], grow: f32, color: Color) {
        let steps = ((b - c).length() / 0.8).ceil().max(1.0) as u32;
        for i in 0..=steps {
            let at = b + (c - b) * (i as f32 / steps as f32);
            self.line(a, at, grow, grow, color);
        }
    }
}

fn draw_one(canvas: &mut Canvas, origin: Vec2, prop: &Prop) {
    let r = prop.radius;
    // Точка опоры на экране и центр предмета над ней
    let base = prop.pos - origin - Vec2::new(0.0, prop.z);
    let c = base - Vec2::new(0.0, r * 0.7);
    let mut pen = Pen {
        canvas,
        opacity: prop.opacity(),
    };
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    // Лёжа на земле: `h` — высота центра над опорой, доли радиуса
    let flat = |dx: f32, dy: f32, h: f32| base + Vec2::new(dx * r, dy * r * DEPTH - h * r);
    match prop.kind {
        PropKind::Ball => ball(&mut pen, base - Vec2::new(0.0, r * 0.9), r, prop.angle),
        PropKind::Apple => apple(&mut pen, c, r, prop.stage),
        PropKind::Acorn => {
            pen.line(
                at(0.0, 0.1),
                at(0.0, 0.95),
                r * 0.72,
                r * 0.12,
                Color(0xB8_7A_3A),
            );
            pen.oval(at(0.0, -0.4), Vec2::new(r * 0.9, r * 0.5), 0.0, BROWN[0]);
            pen.line(at(0.0, -0.8), at(0.12, -1.2), r * 0.1, r * 0.1, BROWN[0]);
        }
        PropKind::Peel => peel(&mut pen, base, r, prop.angle),
        PropKind::Pile => pile(&mut pen, base, r, prop),
        PropKind::Egg => egg(&mut pen, c, r, prop.stage),
        PropKind::Coin => {
            let width = coin_width(prop);
            let rim = Color(0xD9_9A_1F);
            pen.oval(c, Vec2::new(r * width, r), 0.0, rim);
            pen.oval(
                c,
                Vec2::new(r * width * 0.7, r * 0.72),
                0.0,
                Color(0xF5_C5_42),
            );
        }
        PropKind::Bucket => bucket(&mut pen, c, r),
        PropKind::Crumb => {
            pen.circle(flat(0.0, 0.0, 0.5), r, Color(0xD9_B4_7A));
            pen.circle(flat(0.8, 0.5, 0.35), r * 0.6, Color(0xC4_9A_5E));
        }
        PropKind::Pebble => {
            let stone = Color(0x9A_A0_A6);
            pen.oval(flat(0.0, 0.0, 0.5), Vec2::new(r, r * 0.7), 0.0, stone);
            pen.oval(
                flat(1.0, 0.6, 0.25),
                Vec2::new(r * 0.5, r * 0.34),
                0.0,
                Color(0x7F_86_8E),
            );
        }
        PropKind::Feather => feather(&mut pen, c, r, prop.angle),
        PropKind::Arrow => cursor(&mut pen, c, r),
        PropKind::Butterfly => butterfly(&mut pen, c, r, prop.age),
        PropKind::Slipper => slipper(&mut pen, base, r, prop.angle),
    }
}

/// Тапок лежит на земле носком по `angle`: подошва и верх у носка.
fn slipper(pen: &mut Pen, base: Vec2, r: f32, angle: f32) {
    let toe = Vec2::new(angle.cos(), angle.sin() * DEPTH);
    let turn = toe.y.atan2(toe.x);
    let sole = base - Vec2::new(0.0, r * 0.2);
    pen.oval(sole, Vec2::new(r, r * 0.45), turn, Color(0xF2_E3_B3));
    let cap = sole + toe * (r * 0.4) - Vec2::new(0.0, r * 0.25);
    pen.oval(cap, Vec2::new(r * 0.6, r * 0.42), turn, Color(0x3D_8B_D9));
}

fn ball(pen: &mut Pen, c: Vec2, r: f32, angle: f32) {
    pen.circle(c, r, Color(0xE5_53_4B));
    pen.oval(c, Vec2::new(r * 0.98, r * 0.3), angle, Color(0xF2_D0_4B));
}

/// `stage` 0 — целое, 1 — надкусано, 2 — половина, 3 — огрызок.
fn apple(pen: &mut Pen, c: Vec2, r: f32, stage: u8) {
    let red = Color(0xD9_44_2B);
    let stalk = Color(0x6B_4A_2A);
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    match stage {
        0 | 1 => {
            let shrink = if stage == 0 { 1.0 } else { 0.92 };
            for side in [-0.28, 0.28] {
                pen.circle(at(side * shrink, 0.08), r * 0.74 * shrink, red);
            }
            if stage == 1 {
                pen.circle(at(0.62, -0.1), r * 0.42, FLESH);
            }
        }
        2 => {
            pen.circle(at(-0.3, 0.1), r * 0.66, red);
            pen.circle(at(0.18, 0.1), r * 0.5, FLESH);
        }
        _ => {
            pen.line(at(0.0, -0.55), at(0.0, 0.65), r * 0.3, r * 0.3, FLESH);
            pen.circle(at(0.0, -0.62), r * 0.3, red);
            pen.circle(at(0.0, 0.7), r * 0.28, red);
        }
    }
    let top = if stage >= 3 { -0.85 } else { -0.75 };
    pen.line(at(0.0, top), at(0.12, top - 0.5), r * 0.1, r * 0.1, stalk);
    if stage < 2 {
        pen.oval(
            at(0.38, -1.15),
            Vec2::new(r * 0.36, r * 0.18),
            -0.5,
            Color(0x4F_A8_5A),
        );
    }
}

/// Банановая кожура лежит на земле тремя полосами от стебелька.
fn peel(pen: &mut Pen, base: Vec2, r: f32, angle: f32) {
    let centre = base - Vec2::new(0.0, r * 0.25);
    let on_ground = |turn: f32, len: f32| {
        centre + Vec2::new(turn.cos() * len * r, turn.sin() * len * r * DEPTH)
    };
    for (turn, color) in [(-0.5, 0xE0_A9_2E), (2.4, 0xE0_A9_2E), (1.55, 0xF2_C9_4C)] {
        let tip = on_ground(turn + angle, 1.25);
        pen.line(centre, tip, r * 0.38, r * 0.12, Color(color));
    }
    pen.oval(centre, Vec2::new(r * 0.38, r * 0.24), 0.0, YELLOW);
    pen.line(
        centre,
        centre - Vec2::new(-r * 0.1, r * 0.55),
        r * 0.1,
        r * 0.1,
        Color(0x7A_5A_2A),
    );
}

/// Бурая горка в три яруса с завитком; мухи кружат по `age`.
fn pile(pen: &mut Pen, base: Vec2, r: f32, prop: &Prop) {
    let tier = |h: f32, w: f32| (base - Vec2::new(0.0, h * r), Vec2::new(w * r, w * r * 0.5));
    for (color, (h, w)) in BROWN
        .into_iter()
        .zip([(0.1, 0.88), (0.42, 0.62), (0.7, 0.38)])
    {
        let (at, radii) = tier(h, w);
        pen.oval(at, radii, 0.0, color);
    }
    let tip = base - Vec2::new(0.0, r * 0.85);
    pen.line(
        tip,
        tip + Vec2::new(r * 0.16, -r * 0.42),
        r * 0.2,
        r * 0.04,
        BROWN[2],
    );
    let scattered = prop.stage == 1;
    let (reach, rise) = if scattered { (2.1, 2.3) } else { (1.25, 1.55) };
    for fly in 0..3 {
        let turn = prop.age * 5.0 + fly as f32 * TAU / 3.0;
        let bob = (prop.age * 17.0 + fly as f32).sin() * r * 0.08;
        let spot = Vec2::new(
            turn.cos() * r * reach,
            turn.sin() * r * reach * 0.4 - r * rise + bob,
        );
        let size = (r * 0.14).max(1.5);
        pen.circle(base + spot, size, INK);
    }
}

/// `stage` 0 — цела, 1…3 — трещина всё шире, на 3 виден желток.
fn egg(pen: &mut Pen, c: Vec2, r: f32, stage: u8) {
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    let shell = Color(0xF7_F1_E3);
    for (grow, color) in [(1.12, WHITE_EDGE), (1.0, shell)] {
        pen.oval(
            at(0.0, 0.12),
            Vec2::new(r * 0.8, r * 0.88) * grow,
            0.0,
            color,
        );
        pen.oval(
            at(0.0, -0.3),
            Vec2::new(r * 0.62, r * 0.7) * grow,
            0.0,
            color,
        );
    }
    if stage >= 3 {
        pen.oval(
            at(0.0, -0.1),
            Vec2::new(r * 0.42, r * 0.34),
            0.0,
            Color(0xF2_B9_3B),
        );
    }
    let crack = Color(0xA8_97_7A);
    let reach = [0.0, 0.3, 0.55, 0.8][usize::from(stage.min(3))];
    let mut prev = at(-reach, -0.2);
    for step in 1..=(stage.min(3) as i32 * 2) {
        let t = step as f32 / (stage.min(3) as f32 * 2.0);
        let zig = if step % 2 == 0 { -0.25 } else { -0.05 };
        let next = at(-reach + 2.0 * reach * t, zig);
        pen.line(prev, next, r * 0.07, r * 0.07, crack);
        prev = next;
    }
}

fn bucket(pen: &mut Pen, c: Vec2, r: f32) {
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    let steel = Color(0x8F_A3_B8);
    let dark = Color(0x5F_6F_82);
    let arc = [
        at(-0.78, -0.45),
        at(-0.55, -1.15),
        at(0.55, -1.15),
        at(0.78, -0.45),
    ];
    for pair in arc.windows(2) {
        pen.line(pair[0], pair[1], r * 0.07, r * 0.07, dark);
    }
    pen.canvas.fill_round_rect(
        Rect::new(at(-0.68, -0.5), at(0.68, 0.6)),
        r * 0.08,
        steel,
        pen.opacity,
    );
    pen.oval(at(0.0, 0.6), Vec2::new(r * 0.68, r * 0.24), 0.0, steel);
    pen.oval(
        at(0.0, -0.5),
        Vec2::new(r * 0.82, r * 0.28),
        0.0,
        Color(0xA9_BA_CC),
    );
    pen.oval(at(0.0, -0.5), Vec2::new(r * 0.66, r * 0.2), 0.0, dark);
}

/// Белое перо: сперва обводка всего белого, потом заливка.
fn feather(pen: &mut Pen, c: Vec2, r: f32, angle: f32) {
    let (sin, cos) = angle.sin_cos();
    let along = |t: f32| c + Vec2::new(cos * t * r, sin * t * r);
    for (grow, color) in [(1.14, WHITE_EDGE), (1.0, WHITE)] {
        pen.oval(c, Vec2::new(r, r * 0.34) * grow, angle, color);
        pen.line(
            along(-0.9),
            along(-1.45),
            r * 0.08 * grow,
            r * 0.05 * grow,
            color,
        );
    }
}

/// Стрелочка курсора: белая с тёмной обводкой.
fn cursor(pen: &mut Pen, c: Vec2, r: f32) {
    let u = r * 1.5;
    let origin = c - Vec2::new(u * 0.3, u * 0.5);
    let p = |x: f32, y: f32| origin + Vec2::new(x * u, y * u);
    let head = [p(0.0, 0.0), p(0.0, 0.95), p(0.66, 0.68)];
    let ink = (r * 0.13).max(1.0);
    for (grow, color) in [(ink, INK), (ink * 0.3, WHITE)] {
        pen.triangle(head, grow, color);
        pen.line(
            p(0.28, 0.78),
            p(0.5, 1.2),
            0.1 * u + grow * 0.6,
            0.1 * u + grow * 0.6,
            color,
        );
    }
}

fn butterfly(pen: &mut Pen, c: Vec2, r: f32, age: f32) {
    let open = (age * TAU * 3.0).sin().abs().max(0.2);
    let at = |dx: f32, dy: f32| c + Vec2::new(dx * r, dy * r);
    for side in [-1.0, 1.0] {
        let lower = Vec2::new(r * 0.42 * open, r * 0.46);
        pen.oval(
            at(side * 0.3 * open, 0.4),
            lower,
            side * 0.3,
            Color(0xE8_60_2C),
        );
        let upper = Vec2::new(r * 0.62 * open, r * 0.72);
        pen.oval(
            at(side * 0.4 * open, -0.2),
            upper,
            -side * 0.4,
            Color(0xFF_9A_1F),
        );
        pen.line(
            at(0.0, -0.55),
            at(side * 0.4, -1.0),
            r * 0.05,
            r * 0.05,
            INK,
        );
    }
    pen.line(at(0.0, -0.55), at(0.0, 0.6), r * 0.12, r * 0.08, INK);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{
        footprints::Footprints,
        leaves::Leaves,
        mud::Mud,
        particles::{Burst, Particles},
    };
    use crate::render::scene::{Effects, draw_effects, effects_rect, enclosing};
    use crate::rng::Rng;

    const KINDS: [PropKind; 14] = [
        PropKind::Ball,
        PropKind::Apple,
        PropKind::Acorn,
        PropKind::Peel,
        PropKind::Pile,
        PropKind::Egg,
        PropKind::Coin,
        PropKind::Bucket,
        PropKind::Crumb,
        PropKind::Pebble,
        PropKind::Feather,
        PropKind::Arrow,
        PropKind::Butterfly,
        PropKind::Slipper,
    ];
    const SIDE: usize = 300;

    fn effects<'a>(
        props: &'a Props,
        particles: &'a Particles,
        parts: &'a (Footprints, Leaves, Mud),
    ) -> Effects<'a> {
        Effects {
            footprints: &parts.0,
            print_radius: 4.5,
            particles,
            leaves: &parts.1,
            mud: &parts.2,
            mud_prints: &parts.0,
            props,
            scribble: &parts.0,
            flock: &[],
        }
    }

    fn parts() -> (Footprints, Leaves, Mud) {
        (Footprints::new(1, 1.0, 1.0), Leaves::new(1), Mud::new(1))
    }

    /// Пиксели слоя и число закрашенных за пределами охвата эффектов с любой стороны.
    fn paint(props: &Props, particles: &Particles, ui: f32) -> (Vec<u32>, usize) {
        const PAD: i32 = 60;
        let parts = parts();
        let all = effects(props, particles, &parts);
        let rect = enclosing(effects_rect(&all, ui).unwrap());
        let mut pixels = vec![0u32; SIDE * SIDE];
        let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
        let from = Vec2::new((rect.x - PAD) as f32, (rect.y - PAD) as f32);
        draw_effects(&mut canvas, from, &all, ui);
        let inside = |v: i32, len: u32| (PAD..PAD + len as i32).contains(&v);
        let leaked = (0..SIDE * SIDE)
            .filter(|i| {
                let (x, y) = ((i % SIDE) as i32, (i / SIDE) as i32);
                pixels[*i] >> 24 != 0 && !(inside(x, rect.width) && inside(y, rect.height))
            })
            .count();
        (pixels, leaked)
    }

    fn one(kind: PropKind, stage: u8, z: f32) -> Props {
        let mut props = Props::new(2);
        let id = props.spawn(kind, Vec2::new(100.0, 120.0), z, 2.0, 30.0);
        let prop = props.get_mut(id).unwrap();
        (prop.stage, prop.age) = (stage, 0.4);
        props
    }

    #[test]
    fn every_prop_draws_inside_its_rect() {
        let none = Particles::new(1);
        for kind in KINDS {
            for z in [0.0, 30.0] {
                let (pixels, leaked) = paint(&one(kind, 1, z), &none, 2.0);
                assert!(pixels.iter().any(|p| p >> 24 != 0), "{kind:?}");
                assert_eq!(leaked, 0, "{kind:?} z={z}");
            }
        }
    }

    #[test]
    fn grounded_props_cast_a_shadow_except_the_butterfly() {
        for kind in KINDS {
            let mut pixels = vec![0u32; SIDE * SIDE];
            let mut canvas = Canvas::new(&mut pixels, SIDE, SIDE as u32, SIDE as u32).unwrap();
            shadows(&mut canvas, Vec2::ZERO, &one(kind, 0, 0.0), 3);
            let shaded = pixels.iter().any(|p| p >> 24 != 0);
            assert_eq!(shaded, kind != PropKind::Butterfly, "{kind:?}");
        }
    }

    #[test]
    fn stages_change_the_picture() {
        let none = Particles::new(1);
        for kind in [PropKind::Apple, PropKind::Egg] {
            let shots: Vec<_> = (0..=3)
                .map(|stage| paint(&one(kind, stage, 0.0), &none, 2.0).0)
                .collect();
            for (i, a) in shots.iter().enumerate() {
                assert!(shots[i + 1..].iter().all(|b| a != b), "{kind:?} {i}");
            }
        }
        let calm = paint(&one(PropKind::Pile, 0, 0.0), &none, 2.0).0;
        let swarm = paint(&one(PropKind::Pile, 1, 0.0), &none, 2.0).0;
        assert_ne!(calm, swarm);
    }

    #[test]
    fn garbage_input_does_not_panic() {
        let mut props = Props::new(16);
        for kind in KINDS {
            let nan = f32::NAN;
            let id = props.spawn(kind, Vec2::new(nan, 1e30), f32::INFINITY, nan, nan);
            let prop = props.get_mut(id).unwrap();
            (prop.angle, prop.age, prop.stage) = (nan, f32::INFINITY, 255);
        }
        let mut particles = Particles::new(64);
        for burst in Burst::ALL {
            let at = Vec2::new(f32::NAN, 0.0);
            particles.burst(burst, at, f32::NAN, &mut Rng::new(3));
        }
        let parts = parts();
        let all = effects(&props, &particles, &parts);
        let mut pixels = vec![0u32; 64 * 64];
        let mut canvas = Canvas::new(&mut pixels, 64, 64, 64).unwrap();
        draw_effects(&mut canvas, Vec2::ZERO, &all, 2.0);
    }

    #[test]
    fn every_particle_mark_draws_inside_its_rect() {
        let none = Props::new(1);
        for burst in Burst::ALL {
            for ui in [1.0, 2.0] {
                let mut particles = Particles::new(32);
                particles.burst(burst, Vec2::new(150.0, 150.0), ui, &mut Rng::new(8));
                particles.update(0.2);
                let (pixels, leaked) = paint(&none, &particles, ui);
                assert!(pixels.iter().any(|p| p >> 24 != 0), "{burst:?}");
                assert_eq!(leaked, 0, "{burst:?} ui={ui}");
            }
        }
    }
}
