use std::f32::consts::PI;

use super::*;

const DT: f32 = 1.0 / 60.0;
const SCALE: f32 = 1.5;
const START: Vec2 = Vec2::new(500.0, 500.0);
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
const WALK: f32 = 70.0 * SCALE;
const HEADINGS: [Vec2; 4] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(0.0, 1.0),
    Vec2::new(-1.0, 0.0),
    Vec2::new(0.0, -1.0),
];
const GESTURES: [Gesture; 17] = [
    Gesture::Idle,
    Gesture::Walk,
    Gesture::Run,
    Gesture::Look,
    Gesture::React,
    Gesture::Interact,
    Gesture::Celebrate,
    Gesture::Think,
    Gesture::Busy,
    Gesture::Warning,
    Gesture::Error,
    Gesture::Sleep,
    Gesture::WakeUp,
    Gesture::Dragged,
    Gesture::Spawn,
    Gesture::Despawn,
    Gesture::Wave,
];

/// Тело, ход которого ведёт тест; поле названо по гусю — он тут по умолчанию.
struct Rig {
    goose: Creature,
    rng: Rng,
    input: RigInput,
    steps: Vec<Footfall>,
}

impl Rig {
    fn new() -> Self {
        Self::seeded(7)
    }

    fn seeded(seed: u64) -> Self {
        Self::of(RigKind::Goose, seed)
    }

    fn of(kind: RigKind, seed: u64) -> Self {
        Self {
            goose: Creature::new(kind, START, RIGHT, SCALE),
            rng: Rng::new(seed),
            input: RigInput {
                pos: START,
                vel: Vec2::ZERO,
                heading: RIGHT,
                look_at: None,
                gesture: Gesture::Idle,
                progress: 0.5,
                scale: SCALE,
                grounded: true,
                antic: Antic::default(),
            },
            steps: Vec::new(),
        }
    }

    fn go(&mut self, speed: f32, heading: Vec2) {
        self.input.heading = heading;
        self.input.vel = heading * speed;
        self.input.gesture = if speed > 0.0 {
            Gesture::Walk
        } else {
            Gesture::Idle
        };
    }

    /// Шагает `secs` кадрами по `dt`; возвращает упавшие за это время лапы.
    fn run(&mut self, secs: f32, dt: f32) -> Vec<Footfall> {
        let mut falls = Vec::new();
        for _ in 0..(secs / dt).round() as usize {
            self.tick(dt);
            falls.append(&mut self.steps);
        }
        falls
    }

    fn tick(&mut self, dt: f32) {
        self.input.pos += self.input.vel * dt;
        self.goose
            .update(dt, &self.input, &mut self.rng, &mut self.steps);
    }
}

fn walking() -> Rig {
    let mut rig = Rig::new();
    rig.go(WALK, RIGHT);
    rig
}

/// Охватывающие шары частей позы: центр и запас на толщину.
fn parts(pose: &Pose) -> Vec<(Vec3, f32)> {
    let s = pose.scale;
    let thick = 6.0 * s;
    let mut all = vec![
        (
            pose.body.center,
            pose.body.radii.into_iter().fold(0.0, f32::max),
        ),
        (pose.head.center, pose.head.radius),
        (pose.muzzle_tip(), thick),
    ];
    all.extend(pose.neck.iter().map(|p| (*p, thick)));
    all.extend(pose.feet.map(|p| (p, thick)));
    all.extend(pose.fore.into_iter().flatten().map(|p| (p, thick)));
    all.extend(pose.trunk.into_iter().flatten().map(|p| (p, thick)));
    all
}

fn is_finite(pose: &Pose) -> bool {
    parts(pose)
        .iter()
        .all(|(p, r)| p.is_finite() && r.is_finite())
        && pose.pos.x.is_finite()
        && pose.shadow.is_finite()
        && pose.head.eyes_open.is_finite()
        && pose.head.dir.x.is_finite()
}

fn inside_extent(pose: &Pose) -> bool {
    let extent = Creature::extent(SCALE);
    parts(pose).iter().all(|(p, r)| {
        let at = pose.screen(*p) - pose.pos;
        at.x.abs() + r <= extent && at.y.abs() + r <= extent
    })
}

#[test]
fn feet_alternate_and_never_swing_together() {
    let mut rig = walking();
    let mut falls = Vec::new();
    for _ in 0..300 {
        rig.tick(DT);
        assert!(!(rig.goose.feet.swinging(0) && rig.goose.feet.swinging(1)));
        falls.append(&mut rig.steps);
    }
    assert!(falls.len() >= 8, "{}", falls.len());
    assert!(falls.windows(2).all(|pair| pair[0].left != pair[1].left));
}

#[test]
fn planted_feet_do_not_slide() {
    let mut rig = walking();
    rig.run(1.0, DT);
    let ankle = |rig: &Rig, leg: usize| rig.goose.pose().feet[leg].ground();
    let mut checked = 0;
    for _ in 0..300 {
        let before = [0, 1].map(|leg| (ankle(&rig, leg), rig.goose.feet.swinging(leg)));
        rig.tick(DT);
        for (leg, (was, swinging)) in before.into_iter().enumerate() {
            if !swinging && !rig.goose.feet.swinging(leg) {
                assert_eq!(ankle(&rig, leg), was);
                checked += 1;
            }
        }
    }
    assert!(checked > 200, "{checked}");
}

#[test]
fn footfalls_follow_distance_not_step_size() {
    let count = |dt: f32, secs: f32| {
        let mut rig = walking();
        rig.run(secs, dt).len() as i64
    };
    let reference = count(1.0 / 120.0, 4.0);
    assert!(reference >= 8);
    for dt in [DT, 1.0 / 30.0, 0.1] {
        assert!((count(dt, 4.0) - reference).abs() <= 1, "dt {dt}");
    }
    let doubled = count(DT, 8.0);
    assert!(
        (doubled - 2 * reference).abs() <= 2,
        "{doubled} {reference}"
    );
}

#[test]
fn standing_still_settles_both_feet_and_stops_stepping() {
    let mut rig = walking();
    rig.run(2.0, DT);
    rig.go(0.0, RIGHT);
    rig.run(1.5, DT);
    assert!(rig.run(2.0, DT).is_empty());
    assert!(!rig.goose.feet.swinging(0) && !rig.goose.feet.swinging(1));
    for foot in rig.goose.pose().feet {
        assert!(foot.ground().distance(rig.input.pos) < 8.0 * SCALE);
        assert_eq!(foot.z, 0.0);
    }
}

#[test]
fn turning_in_place_replants_feet_in_small_steps() {
    let mut rig = Rig::new();
    rig.run(1.0, DT);
    rig.input.heading = Vec2::new(0.0, 1.0);
    let falls = rig.run(1.5, DT);
    assert!(falls.len() >= 2, "{}", falls.len());
    assert!(rig.run(1.0, DT).is_empty());
}

#[test]
fn dragged_goose_dangles_and_leaves_no_footfalls() {
    let mut rig = walking();
    rig.run(1.0, DT);
    rig.input.grounded = false;
    rig.input.gesture = Gesture::Dragged;
    assert!(rig.run(2.0, DT).is_empty());
    let pose = rig.goose.pose();
    assert!(
        pose.feet
            .iter()
            .all(|f| f.z > 0.0 && f.z < pose.body.center.z)
    );
    assert!(pose.shadow < 0.6);
    rig.input.grounded = true;
    rig.go(0.0, RIGHT);
    rig.tick(DT);
    assert!(rig.steps.is_empty());
    for foot in rig.goose.pose().feet {
        assert!(foot.ground().distance(rig.input.pos) < 8.0 * SCALE);
    }
}

#[test]
fn reset_replants_feet_under_the_body() {
    let mut rig = walking();
    rig.run(1.0, DT);
    let to = Vec2::new(100.0, 900.0);
    rig.goose.reset(to, Vec2::new(0.0, -1.0), 2.0);
    assert!(!rig.goose.feet.swinging(0) && !rig.goose.feet.swinging(1));
    let pose = rig.goose.pose();
    assert_eq!(pose.pos, to);
    assert_eq!(pose.scale, 2.0);
    assert!(
        pose.feet
            .iter()
            .all(|f| f.ground().distance(to) < 8.0 * 2.0)
    );
}

#[test]
fn pose_stays_finite_for_odd_steps_and_inputs() {
    let mut rig = walking();
    for dt in [0.0, 1e-9, 0.25, 5.0, 1e9, f32::NAN, -1.0, f32::INFINITY] {
        rig.tick(dt);
        assert!(is_finite(rig.goose.pose()), "dt {dt}");
    }
    let nan = f32::NAN;
    let broken = RigInput {
        pos: Vec2::new(nan, 1.0),
        vel: Vec2::new(f32::INFINITY, nan),
        heading: Vec2::new(nan, nan),
        look_at: Some(Vec2::new(nan, nan)),
        gesture: Gesture::Celebrate,
        progress: nan,
        scale: nan,
        grounded: true,
        antic: Antic::default(),
    };
    for _ in 0..10 {
        rig.goose.update(DT, &broken, &mut rig.rng, &mut rig.steps);
        assert!(is_finite(rig.goose.pose()));
    }
    let zero_heading = RigInput {
        heading: Vec2::ZERO,
        scale: -3.0,
        progress: 7.0,
        ..rig.input
    };
    rig.goose
        .update(DT, &zero_heading, &mut rig.rng, &mut rig.steps);
    assert!(is_finite(rig.goose.pose()));
    let reset = Creature::new(RigKind::Pig, Vec2::new(nan, nan), Vec2::ZERO, nan);
    assert!(is_finite(reset.pose()));
}

#[test]
fn head_turns_toward_what_it_looks_at() {
    for heading in HEADINGS {
        for target in HEADINGS {
            let mut plain = Rig::new();
            plain.goose.reset(START, heading, SCALE);
            plain.input.heading = heading;
            let mut looking = Rig::new();
            looking.goose.reset(START, heading, SCALE);
            looking.input.heading = heading;
            looking.input.look_at = Some(START + target * 300.0);
            plain.run(1.5, DT);
            looking.run(1.5, DT);
            let turned = looking.goose.pose().head.dir.dot(target);
            let ahead = plain.goose.pose().head.dir.dot(target);
            // Прямо назад голова не оборачивается: предел поворота
            if heading.dot(target) > -0.5 {
                assert!(turned > ahead - 1e-3, "{heading:?} {target:?}");
            }
        }
    }
    let mut rig = Rig::new();
    rig.input.look_at = Some(START + Vec2::new(0.0, -300.0));
    rig.run(1.5, DT);
    assert!(rig.goose.pose().head.dir.y < -0.9);
}

#[test]
fn everything_stays_within_extent() {
    for kind in RigKind::ALL {
        for heading in HEADINGS {
            for gesture in GESTURES {
                for step in 0..=10 {
                    let mut rig = Rig::of(kind, 7);
                    rig.goose.reset(START, heading, SCALE);
                    rig.input.heading = heading;
                    rig.input.gesture = gesture;
                    rig.input.grounded = gesture != Gesture::Dragged;
                    rig.input.progress = step as f32 / 10.0;
                    if matches!(gesture, Gesture::Walk | Gesture::Run) {
                        rig.input.vel = heading * (230.0 * SCALE);
                    }
                    for _ in 0..90 {
                        rig.tick(DT);
                        let pose = rig.goose.pose();
                        assert!(is_finite(pose), "{kind:?} {gesture:?} {heading:?}");
                        assert!(inside_extent(pose), "{kind:?} {gesture:?} {heading:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn pig_trots_on_four_feet_that_do_not_slide() {
    let mut pig = Rig::of(RigKind::Pig, 7);
    pig.go(WALK, RIGHT);
    pig.run(1.0, DT);
    let planted = |pig: &Rig| {
        let (pose, body) = (pig.goose.pose(), &pig.goose);
        let fore = body.fore.as_ref().unwrap();
        [
            (!body.feet.swinging(0)).then_some(pose.feet[0]),
            (!body.feet.swinging(1)).then_some(pose.feet[1]),
            (!fore.swinging(0)).then_some(pose.fore.unwrap()[0]),
            (!fore.swinging(1)).then_some(pose.fore.unwrap()[1]),
        ]
    };
    let (mut falls, mut checked, mut diagonal) = (0, 0, 0);
    for _ in 0..300 {
        let before = planted(&pig);
        pig.tick(DT);
        falls += pig.steps.drain(..).count();
        let fore = pig.goose.fore.as_ref().unwrap();
        // Рысь: задняя нога идёт вместе с передней другой стороны
        diagonal += usize::from(pig.goose.feet.swinging(0) && fore.swinging(1));
        for (was, now) in before.into_iter().zip(planted(&pig)) {
            if let (Some(was), Some(now)) = (was, now) {
                assert_eq!(was, now);
                checked += 1;
            }
        }
    }
    assert!(falls >= 16, "{falls}");
    assert!(checked > 400, "{checked}");
    assert!(diagonal > 20, "{diagonal}");
}

#[test]
fn pig_never_walks_sideways_but_goose_may() {
    let up = Vec2::new(0.0, -1.0);
    let ahead = |kind| {
        let mut rig = Rig::of(kind, 7);
        // Мозг держит сторону «вправо», а идти надо вверх
        rig.input.heading = RIGHT;
        rig.input.vel = up * WALK;
        rig.input.gesture = Gesture::Walk;
        rig.run(1.5, DT);
        rig.goose.pose().body.forward.dot(up)
    };
    assert!(ahead(RigKind::Pig) > 0.99);
    assert!(ahead(RigKind::Goose) < 0.1);
    // Встала — разворачивается, куда велено
    let mut pig = Rig::of(RigKind::Pig, 7);
    pig.input.vel = up * WALK;
    pig.run(1.0, DT);
    pig.go(0.0, RIGHT);
    pig.run(1.5, DT);
    assert!(pig.goose.pose().body.forward.dot(RIGHT) > 0.99);
}

#[test]
fn tail_tip_lags_on_start_and_turn_then_settles() {
    let mut raccoon = Rig::of(RigKind::Raccoon, 7);
    raccoon.go(WALK, RIGHT);
    raccoon.run(0.1, DT);
    assert!(raccoon.goose.pose().tail.x < -SCALE);
    raccoon.run(2.0, DT);
    assert!(raccoon.goose.pose().tail.length() < 0.2 * SCALE);
    // Поворот налево отбрасывает кончик вправо
    raccoon.go(0.0, Vec2::new(0.0, -1.0));
    raccoon.run(0.15, DT);
    assert!(raccoon.goose.pose().tail.y > 1.0 * SCALE);
    raccoon.run(3.0, DT);
    assert!(raccoon.goose.pose().tail.length() < 0.2 * SCALE);
}

#[test]
fn pig_head_sits_on_the_body_and_dangles_on_four_legs() {
    let mut pig = Rig::of(RigKind::Pig, 7);
    pig.go(230.0 * SCALE, RIGHT);
    pig.run(1.0, DT);
    pig.go(0.0, RIGHT);
    for _ in 0..60 {
        pig.tick(DT);
        let pose = pig.goose.pose();
        // Шеи нет: голова не отрывается от тела даже на резкой остановке
        let gap = (pose.head.center - pose.body.center).length();
        assert!(gap < pose.body.radii[0] + pose.head.radius, "{gap}");
    }
    pig.input.grounded = false;
    pig.input.gesture = Gesture::Dragged;
    pig.run(0.5, DT);
    let pose = pig.goose.pose();
    let feet = pose.feet.into_iter().chain(pose.fore.unwrap());
    assert!(feet.clone().all(|f| f.z > 0.0 && f.z < pose.body.center.z));
    let along = |f: Vec3| (f.ground() - pose.pos).dot(RIGHT);
    assert!(along(pose.feet[0]) < 0.0 && along(pose.fore.unwrap()[0]) > 0.0);
}

#[test]
fn same_seed_same_poses() {
    let script = |seed| {
        let mut rig = Rig::seeded(seed);
        let mut poses = Vec::new();
        for frame in 0..600 {
            rig.input.gesture = GESTURES[frame / 36 % GESTURES.len()];
            rig.input.progress = (frame % 36) as f32 / 36.0;
            rig.input.heading = HEADINGS[frame / 150 % 4];
            rig.input.vel = rig.input.heading * if frame % 200 < 100 { WALK } else { 0.0 };
            rig.tick(DT);
            poses.push(rig.goose.pose().clone());
        }
        poses
    };
    assert_eq!(script(3), script(3));
    assert_ne!(script(3), script(4));
}

#[test]
fn every_gesture_changes_the_pose() {
    let settled = |gesture| {
        let mut rig = Rig::new();
        rig.input.gesture = gesture;
        rig.run(1.5, DT);
        rig.goose.pose().clone()
    };
    let idle = settled(Gesture::Idle);
    for gesture in GESTURES.into_iter().filter(|g| *g != Gesture::Idle) {
        assert_ne!(settled(gesture), idle, "{gesture:?}");
    }
}

#[test]
fn spawn_pops_in_and_despawn_shrinks() {
    let size = |gesture, progress| {
        let mut rig = Rig::new();
        rig.input.gesture = gesture;
        rig.input.progress = progress;
        rig.tick(DT);
        rig.goose.pose().scale
    };
    assert_eq!(size(Gesture::Spawn, 0.0), 0.0);
    assert!((size(Gesture::Spawn, 1.0) - SCALE).abs() < 1e-4);
    assert!(size(Gesture::Despawn, 1.0).abs() < 1e-4);
    assert!((size(Gesture::Despawn, 0.0) - SCALE).abs() < 1e-4);
}

#[test]
fn body_turn_does_not_depend_on_step_size() {
    let yaw_after = |dt: f32| {
        let mut rig = Rig::new();
        rig.input.heading = Vec2::new(0.0, -1.0);
        rig.run(0.2, dt);
        rig.goose.yaw
    };
    assert!((yaw_after(DT) - yaw_after(0.1)).abs() < 0.02);
    assert!((yaw_after(DT) - yaw_after(1.0 / 120.0)).abs() < 0.02);
}

#[test]
fn eyes_blink_now_and_then() {
    let mut rig = Rig::new();
    let mut closed = 0;
    for _ in 0..1200 {
        rig.tick(DT);
        if rig.goose.pose().head.eyes_open < 0.3 {
            closed += 1;
        }
    }
    assert!(closed > 5 && closed < 200, "{closed}");
}

#[test]
fn muzzle_tip_is_ahead_of_the_head_on_screen() {
    let rig = Rig::new();
    let pose = rig.goose.pose();
    assert!(rig.goose.head().x > pose.head.center.x);
    assert!(rig.goose.head().y < START.y);
}

fn with_antic(kind: RigKind, antic: Antic, look: Look) -> Rig {
    let mut rig = Rig::of(kind, 7);
    rig.input.antic = antic;
    rig.goose.set_look(look);
    rig.run(1.0, DT);
    rig
}

fn antic(edit: impl FnOnce(&mut Antic)) -> Antic {
    let mut antic = Antic::default();
    edit(&mut antic);
    antic
}

fn bloated() -> Look {
    Look {
        bloat: 1.0,
        ..Look::default()
    }
}

#[test]
fn on_its_back_the_feet_are_above_the_body_and_the_head_is_low() {
    for kind in RigKind::ALL {
        let rig = with_antic(kind, antic(|a| a.roll = PI), Look::default());
        let pose = rig.goose.pose();
        let feet = pose.feet.into_iter().chain(pose.fore.into_iter().flatten());
        assert!(
            feet.into_iter().all(|f| f.z > pose.body.center.z),
            "{kind:?}"
        );
        assert!(pose.head.center.z < pose.body.center.z, "{kind:?}");
        assert!(pose.body.center.z > 0.0 && pose.body.center.z < 12.0 * SCALE);
    }
}

#[test]
fn on_its_side_the_feet_stick_out_sideways() {
    for kind in RigKind::ALL {
        let rig = with_antic(kind, antic(|a| a.roll = PI / 2.0), Look::default());
        let pose = rig.goose.pose();
        let aside = (pose.feet[0].ground() - pose.body.center.ground()).length();
        assert!(aside > pose.body.radii[1], "{kind:?}");
        assert!(pose.body.center.z < pose.body.radii[1] + 0.5, "{kind:?}");
    }
}

#[test]
fn lifted_body_takes_its_feet_and_shadow_along() {
    for kind in RigKind::ALL {
        let ground = with_antic(kind, Antic::default(), Look::default());
        let rig = with_antic(kind, antic(|a| a.lift = 20.0), Look::default());
        let pose = rig.goose.pose();
        let feet = pose.feet.into_iter().chain(pose.fore.into_iter().flatten());
        assert!(feet.into_iter().all(|f| f.z > 10.0 * SCALE), "{kind:?}");
        assert!(pose.shadow < ground.goose.pose().shadow * 0.5, "{kind:?}");
    }
}

#[test]
fn squash_and_bloat_change_the_body() {
    for kind in RigKind::ALL {
        let squashed = |x, y| antic(|a| a.squash = Vec2::new(x, y));
        let plain = with_antic(kind, Antic::default(), Look::default());
        let flat = with_antic(kind, squashed(1.6, 0.25), Look::default());
        let tall = with_antic(kind, squashed(0.9, 1.2), Look::default());
        let fat = with_antic(kind, Antic::default(), bloated());
        // Полуоси вдоль, вбок и по росту: у тела торчком рост — продольная ось
        let upright = |body: &Body| {
            let [along, side, up] = body.radii;
            if body.pitch > PI / 4.0 {
                [up, side, along]
            } else {
                [along, side, up]
            }
        };
        let radii = |rig: &Rig| upright(&rig.goose.pose().body);
        let base = radii(&plain);
        assert!(radii(&flat)[1] > base[1] * 1.5 && radii(&flat)[2] < base[2] * 0.3);
        assert!(radii(&tall)[2] > base[2] * 1.1 && radii(&tall)[0] < base[0]);
        assert!(radii(&fat).iter().zip(base).all(|(f, b)| *f > b * 1.3));
        // Низ тела не проваливается под землю и не отрывается от неё
        for rig in [&flat, &tall, &fat] {
            let body = rig.goose.pose().body;
            let bottom = body.center.z - upright(&body)[2];
            assert!(bottom > -0.5 * SCALE, "{kind:?}");
            assert!(bottom < 4.0 * SCALE, "{kind:?}");
        }
    }
}

#[test]
fn lying_or_airborne_body_leaves_no_footprints() {
    for kind in RigKind::ALL {
        for case in [antic(|a| a.roll = PI), antic(|a| a.lift = 20.0)] {
            let mut rig = Rig::of(kind, 7);
            rig.go(WALK, RIGHT);
            rig.run(1.0, DT);
            rig.input.antic = case;
            rig.run(0.4, DT);
            assert!(rig.run(1.5, DT).is_empty(), "{kind:?}");
        }
    }
}

#[test]
fn fast_spin_keeps_up_with_the_angle() {
    let mut rig = Rig::of(RigKind::Pig, 7);
    let mut spin = 0.0;
    for _ in 0..120 {
        spin += 15.0 * DT;
        rig.input.antic.spin = spin;
        rig.tick(DT);
        let lag = wrap_angle(rig.goose.yaw - spin);
        assert!(lag.abs() < 0.3, "{lag}");
    }
}

#[test]
fn antic_garbage_stays_finite() {
    for kind in RigKind::ALL {
        for v in [f32::NAN, f32::INFINITY, -1.0e30, 1.0e30, 0.0, -3.0] {
            let mut rig = Rig::of(kind, 7);
            rig.input.antic = Antic {
                lift: v,
                squash: Vec2::new(v, v),
                roll: v,
                spin: v,
                ..Antic::default()
            };
            rig.goose.set_look(Look {
                bloat: v,
                ..Look::default()
            });
            rig.run(0.5, DT);
            assert!(is_finite(rig.goose.pose()), "{kind:?} {v}");
        }
    }
}

#[test]
fn poses_with_antic_stay_within_extent() {
    let cases = [
        antic(|a| a.roll = PI),
        antic(|a| a.roll = PI / 2.0),
        antic(|a| a.squash = Vec2::new(1.6, 0.25)),
        antic(|a| a.squash = Vec2::new(0.9, 1.2)),
        antic(|a| a.lift = 25.0),
    ];
    for kind in RigKind::ALL {
        for heading in HEADINGS {
            for case in cases {
                for look in [Look::default(), bloated()] {
                    let mut rig = Rig::of(kind, 7);
                    rig.goose.reset(START, heading, SCALE);
                    rig.input.heading = heading;
                    rig.input.antic = case;
                    rig.goose.set_look(look);
                    rig.run(1.0, DT);
                    let pose = rig.goose.pose();
                    assert!(
                        is_finite(pose) && inside_extent(pose),
                        "{kind:?} {heading:?} {look:?} {case:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn elephant_trunk_hangs_rises_with_the_head_and_lies_on_the_ground() {
    let tip = |head_pitch, neck_up| {
        let rig = with_antic(
            RigKind::Elephant,
            antic(|a| {
                a.neck_forward = 12.0 * neck_up / -15.0;
                a.neck_up = neck_up;
                a.head_pitch = head_pitch;
            }),
            Look::default(),
        );
        let pose = rig.goose.pose();
        assert_eq!(rig.goose.head(), pose.screen(pose.muzzle_tip()));
        (pose.muzzle_tip() - Vec3::on_ground(START, 0.0)) * (1.0 / SCALE)
    };
    let hanging = tip(0.0, 0.0);
    let raised = tip(1.0, 0.0);
    let lowered = tip(-0.6, -15.0);
    assert!(hanging.z > 2.0 && hanging.z < 12.0, "{hanging:?}");
    assert!(raised.z > 30.0, "{raised:?}");
    // Так сценки слоника ставят тело к предмету (`TRUNK_REACH`)
    assert!(
        lowered.z < 2.0 && (lowered.x - 32.0).abs() < 1.0,
        "{lowered:?}"
    );
}

#[test]
fn elephant_trunk_tip_swings_behind_when_starting_off() {
    let mut rig = Rig::of(RigKind::Elephant, 7);
    rig.run(1.0, DT);
    let rest = rig.goose.pose().trunk.unwrap();
    rig.go(230.0 * SCALE, RIGHT);
    rig.tick(DT);
    rig.tick(DT);
    let moving = rig.goose.pose().trunk.unwrap();
    let lag = |[root, tip]: [Vec3; 2]| (tip - root).x;
    assert!(
        lag(moving) < lag(rest) - 0.5,
        "{} {}",
        lag(moving),
        lag(rest)
    );
}

#[test]
fn penguin_waddles_with_short_steps_and_the_goose_does_not() {
    let walk = |kind| {
        let mut rig = Rig::of(kind, 7);
        rig.go(WALK, RIGHT);
        rig.run(1.0, DT);
        let mut rocks = Vec::new();
        let falls = (0..120)
            .map(|_| {
                rig.tick(DT);
                rocks.push(rig.goose.pose().body.rock);
                rig.steps.drain(..).count()
            })
            .sum::<usize>();
        let (low, high) = rocks
            .iter()
            .fold((0.0f32, 0.0f32), |(l, h), r| (l.min(*r), h.max(*r)));
        (falls, low, high)
    };
    let (penguin, low, high) = walk(RigKind::Penguin);
    assert!(low < -0.15 && high > 0.15, "{low} {high}");
    let (goose, low, high) = walk(RigKind::Goose);
    assert_eq!((low, high), (0.0, 0.0));
    assert!(penguin > goose * 3 / 2, "{penguin} {goose}");
}
