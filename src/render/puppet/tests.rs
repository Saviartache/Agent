use super::*;
use crate::assets::Character;
use crate::render::creature;
use crate::rig::{Antic, Creature, Eyes, Gesture, Look, RigInput, RigKind};
use crate::rng::Rng;

const SCALE: f32 = 1.5;
const AT: Vec2 = Vec2::new(300.5, 400.25);
/// Запас вокруг холста тела, чтобы видеть утечки за его край.
const MARGIN: usize = 40;
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
const TOWARD: Vec2 = Vec2::new(0.0, 1.0);
const AWAY: Vec2 = Vec2::new(0.0, -1.0);
/// Четыре прямые стороны и четыре вполоборота.
const HEADINGS: [Vec2; 8] = [
    RIGHT,
    Vec2::new(-1.0, 0.0),
    TOWARD,
    AWAY,
    Vec2::new(0.7, 0.7),
    Vec2::new(-0.7, 0.7),
    Vec2::new(-0.7, -0.7),
    Vec2::new(0.7, -0.7),
];
const WHITE: u32 = 0xFFFF_FFFF;
const INK: u32 = 0xFF1B_1F29;

fn puppet(kind: RigKind) -> Puppet {
    let character = Character::builtin(kind).unwrap();
    Puppet::new(Arc::clone(character.skeleton().unwrap()))
}

fn pose(kind: RigKind, scale: f32, heading: Vec2, antic: Antic, look: Look) -> Pose {
    let mut body = Creature::new(kind, AT, heading, scale);
    body.set_look(look);
    let input = RigInput {
        pos: AT,
        vel: Vec2::ZERO,
        heading,
        look_at: None,
        gesture: Gesture::Idle,
        progress: 0.5,
        scale,
        grounded: true,
        antic,
    };
    let (mut rng, mut steps) = (Rng::new(7), Vec::new());
    for _ in 0..90 {
        body.update(1.0 / 60.0, &input, &mut rng, &mut steps);
    }
    body.pose().clone()
}

fn standing(kind: RigKind, heading: Vec2) -> Pose {
    pose(kind, SCALE, heading, Antic::default(), Look::default())
}

/// Тело без тени и мозаики на холсте персонажа с запасом `MARGIN` со всех сторон.
fn render(pose: &Pose, puppet: &mut Puppet) -> (Vec<u32>, usize, usize) {
    let rect = creature::rect(pose, Creature::extent(pose.scale));
    let (w, h) = (
        rect.width as usize + 2 * MARGIN,
        rect.height as usize + 2 * MARGIN,
    );
    let mut pixels = vec![0u32; w * h];
    let mut canvas = Canvas::new(&mut pixels, w, w as u32, h as u32).unwrap();
    let origin = Vec2::new(rect.x as f32, rect.y as f32) - Vec2::new(1.0, 1.0) * MARGIN as f32;
    let mut still = pose.clone();
    still.shadow = 0.0;
    puppet.paint(&mut canvas, origin, &still);
    (pixels, w, h)
}

fn shot(kind: RigKind, heading: Vec2, antic: Antic, look: Look) -> Vec<u32> {
    render(&pose(kind, SCALE, heading, antic, look), &mut puppet(kind)).0
}

fn opaque(pixels: &[u32]) -> usize {
    pixels.iter().filter(|pixel| **pixel >> 24 != 0).count()
}

fn count(pixels: &[u32], color: u32) -> usize {
    pixels.iter().filter(|pixel| **pixel == color).count()
}

/// Накладки сценок и следы на теле, которыми проверяется каждый вид.
fn looks() -> Vec<(Antic, Look)> {
    let plain = Look::default();
    let antic = Antic::default;
    vec![
        (antic(), plain),
        (
            Antic {
                lids: 1.0,
                tuck: 1.0,
                ..antic()
            },
            plain,
        ),
        (
            Antic {
                eyes: Eyes::Crossed,
                roll: 1.2,
                tongue: true,
                ..antic()
            },
            plain,
        ),
        (
            Antic {
                eyes: Eyes::Wide,
                lift: 25.0,
                mouth: 1.0,
                ..antic()
            },
            plain,
        ),
        (
            Antic {
                squash: Vec2::new(1.6, 0.25),
                ..antic()
            },
            plain,
        ),
        (
            Antic {
                wings: 1.0,
                flap: 0.2,
                knot: 1.0,
                ..antic()
            },
            plain,
        ),
        (
            Antic {
                roll: std::f32::consts::PI,
                ..antic()
            },
            Look {
                mud: 1.0,
                snout: 1.0,
                bloat: 1.0,
            },
        ),
    ]
}

#[test]
fn every_body_draws_inside_its_canvas_in_any_pose() {
    for kind in RigKind::ALL {
        let mut puppet = puppet(kind);
        for heading in HEADINGS {
            for (antic, look) in looks() {
                let pose = pose(kind, SCALE, heading, antic, look);
                let (pixels, w, h) = render(&pose, &mut puppet);
                assert!(opaque(&pixels) > 300, "{kind:?} {heading:?} {antic:?}");
                let outside = pixels.iter().enumerate().filter(|(i, pixel)| {
                    let (x, y) = (i % w, i / w);
                    let inside =
                        (MARGIN..w - MARGIN).contains(&x) && (MARGIN..h - MARGIN).contains(&y);
                    **pixel >> 24 != 0 && !inside
                });
                assert_eq!(outside.count(), 0, "{kind:?} {heading:?} {antic:?}");
            }
        }
    }
}

#[test]
fn every_screen_scale_gives_the_same_body() {
    for kind in RigKind::ALL {
        let mut puppet = puppet(kind);
        // Площадь тела растёт как квадрат масштаба: ничего не пропадает и не раздувается
        let mut area = |scale: f32| {
            let pose = pose(kind, scale, RIGHT, Antic::default(), Look::default());
            opaque(&render(&pose, &mut puppet).0) as f32 / (scale * scale)
        };
        let base = area(1.5);
        for scale in [1.0, 2.0, 3.0] {
            let ratio = area(scale) / base;
            assert!((0.93..1.07).contains(&ratio), "{kind:?} {scale} {ratio}");
        }
    }
}

#[test]
fn eyes_look_out_from_the_front_and_hide_behind_the_head() {
    for kind in RigKind::ALL {
        let front = shot(kind, TOWARD, Antic::default(), Look::default());
        let back = shot(kind, AWAY, Antic::default(), Look::default());
        assert!(count(&front, INK) > 2, "{kind:?} {}", count(&front, INK));
        // У енота чернилами ещё и нос: со спины не видно ни его, ни зрачков
        assert_eq!(count(&back, INK), 0, "{kind:?}");
        let asleep = Antic {
            lids: 1.0,
            ..Antic::default()
        };
        let closed = shot(kind, TOWARD, asleep, Look::default());
        assert!(count(&closed, INK) < count(&front, INK), "{kind:?}");
    }
}

#[test]
fn eye_states_change_the_eyes() {
    for kind in RigKind::ALL {
        let eyes = |eyes: Eyes| {
            let antic = Antic {
                eyes,
                ..Antic::default()
            };
            shot(kind, TOWARD, antic, Look::default())
        };
        let (normal, wide, crossed) = (eyes(Eyes::Normal), eyes(Eyes::Wide), eyes(Eyes::Crossed));
        // У гуся распахнутый глаз белый на белой голове: больше становится только кайма
        if kind != RigKind::Goose {
            assert!(count(&wide, WHITE) > count(&normal, WHITE), "{kind:?}");
        }
        assert_ne!(wide, normal, "{kind:?}");
        assert_ne!(crossed, normal, "{kind:?}");
        assert_ne!(crossed, wide, "{kind:?}");
    }
}

#[test]
fn tucked_legs_and_open_mouths_show() {
    for kind in RigKind::ALL {
        let plain = shot(kind, RIGHT, Antic::default(), Look::default());
        let tucked = Antic {
            tuck: 1.0,
            ..Antic::default()
        };
        assert_ne!(
            shot(kind, RIGHT, tucked, Look::default()),
            plain,
            "{kind:?}"
        );
        let gaping = Antic {
            mouth: 1.0,
            tongue: true,
            ..Antic::default()
        };
        assert_ne!(
            shot(kind, TOWARD, gaping, Look::default()),
            shot(kind, TOWARD, Antic::default(), Look::default()),
            "{kind:?}"
        );
    }
}

#[test]
fn wings_ears_and_flippers_spread() {
    for kind in [RigKind::Goose, RigKind::Penguin, RigKind::Elephant] {
        let spread = Antic {
            wings: 1.0,
            flap: 0.25,
            ..Antic::default()
        };
        let (open, shut) = (
            shot(kind, AWAY, spread, Look::default()),
            shot(kind, AWAY, Antic::default(), Look::default()),
        );
        let width = |pixels: &[u32]| {
            let side = (pixels.len() as f32).sqrt() as usize;
            let columns = (0..side).filter(|x| (0..side).any(|y| pixels[y * side + x] >> 24 != 0));
            columns.count()
        };
        assert!(
            width(&open) > width(&shut) + 6,
            "{kind:?} {} {}",
            width(&open),
            width(&shut)
        );
    }
}

#[test]
fn goose_neck_ties_into_a_knot() {
    let knotted = Antic {
        knot: 1.0,
        ..Antic::default()
    };
    let (plain, knot) = (
        shot(RigKind::Goose, RIGHT, Antic::default(), Look::default()),
        shot(RigKind::Goose, RIGHT, knotted, Look::default()),
    );
    assert!(count(&knot, WHITE) > count(&plain, WHITE) + 40);
}

#[test]
fn mud_shows_on_the_pig_and_on_beaks() {
    let muddy = Look {
        mud: 1.0,
        snout: 1.0,
        ..Look::default()
    };
    const MUD: u32 = 0xFF7A_5534;
    for kind in [RigKind::Pig, RigKind::Goose, RigKind::Penguin] {
        let clean = shot(kind, TOWARD, Antic::default(), Look::default());
        let dirty = shot(kind, TOWARD, Antic::default(), muddy);
        assert_eq!(count(&clean, MUD), 0, "{kind:?}");
        assert!(count(&dirty, MUD) > 2, "{kind:?}");
    }
}

#[test]
fn four_legged_bodies_stand_on_legs_hidden_under_the_belly() {
    for (kind, leg) in [
        (RigKind::Pig, 0xFFC4_5F69),
        (RigKind::Elephant, 0xFF71_87A3),
        (RigKind::Raccoon, 0xFF4E_4842),
    ] {
        let pose = standing(kind, RIGHT);
        let (pixels, w, _) = render(&pose, &mut puppet(kind));
        let rect = creature::rect(&pose, Creature::extent(SCALE));
        // Ноги видны только ниже центра тела: верх ноги на бок не ложится
        let center = pose.screen(pose.body.center).y - rect.y as f32 + MARGIN as f32;
        let (above, below) =
            pixels
                .iter()
                .enumerate()
                .fold((0, 0), |(above, below), (i, pixel)| {
                    match (*pixel == leg, ((i / w) as f32) < center) {
                        (true, true) => (above + 1, below),
                        (true, false) => (above, below + 1),
                        _ => (above, below),
                    }
                });
        assert!(below > 20 && above == 0, "{kind:?} {above} {below}");
    }
}

#[test]
fn figure_keeps_one_puppet_per_skeleton() {
    let pose = standing(RigKind::Pig, RIGHT);
    let pig = puppet(RigKind::Pig);
    let skeleton = Arc::clone(pig.skeleton());
    let mut figure = Figure::default();
    let mut shoot = |skeleton: &Arc<Skeleton>| {
        let mut pixels = vec![0u32; 260 * 260];
        let mut canvas = Canvas::new(&mut pixels, 260, 260, 260).unwrap();
        figure.draw(
            &mut canvas,
            pose.pos - Vec2::new(130.0, 130.0),
            &pose,
            skeleton,
        );
        pixels
    };
    let first = shoot(&skeleton);
    assert!(opaque(&first) > 900);
    assert_eq!(shoot(&skeleton), first);
    // Сменился набор — кукла новая, картинка та же
    assert_eq!(shoot(&Arc::new((*skeleton).clone())), first);
    // Другой вид — другое тело
    assert_ne!(shoot(puppet(RigKind::Goose).skeleton()), first);
}

#[test]
fn broken_poses_stay_quiet() {
    for kind in RigKind::ALL {
        let mut puppet = puppet(kind);
        let mut gone = standing(kind, RIGHT);
        gone.zoomed(0.0);
        assert_eq!(opaque(&render(&gone, &mut puppet).0), 0, "{kind:?}");
        let mut broken = standing(kind, RIGHT);
        broken.body.center.x = f32::NAN;
        broken.head.center.z = f32::INFINITY;
        render(&broken, &mut puppet);
    }
}
