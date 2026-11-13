// Сценарии сценок (goose_a): файл подключается в модули тел из `sim/tests.rs`.
// Сценки гуся: удар о край, гогот, шипение, строевой шаг, юз, полёт, чистка перьев.

/// Живой гусь с зерном `seed`: исходы сценок случайны, перебираем зёрна.
fn goose_a_world(seed: u64) -> World {
    let mut world = World::new(&config());
    world.sim = Sim::new(character(), &config(), screen(), 96, seed);
    world.sim.command(Command::Spawn(Some(SPAWN_AT)));
    assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Spawned)));
    world.events.clear();
    world
}

/// Каждый тик зовёт `look`; `true`, если сценка доиграна за `secs`. Гусь не покидает экран,
/// системный курсор не двигается.
fn goose_a_play(world: &mut World, stunt: Stunt, secs: f32, mut look: impl FnMut(&World)) -> bool {
    world.sim.command(Command::Stage(stunt));
    let hull = screen().hull();
    for _ in 0..(secs / DT) as usize {
        let moved = world.tick();
        assert!(moved.is_none(), "{stunt:?}: курсор тронут");
        let at = world.sim.position();
        assert_eq!(hull.clamp(at, 0.0), at, "{stunt:?}: ушёл за экран");
        look(world);
        if finished(stunt)(world) {
            return true;
        }
    }
    false
}

fn goose_a_feathers(world: &World) -> usize {
    world
        .sim
        .props
        .iter()
        .filter(|prop| prop.kind == crate::effects::props::PropKind::Feather)
        .count()
}

fn goose_a_rings(world: &World) -> bool {
    world
        .sim
        .particles()
        .iter()
        .any(|p| p.shape == crate::effects::particles::Shape::Ring)
}

#[test]
fn goose_a_wall_slam_hits_the_edge_and_loses_feathers() {
    let mut world = goose_a_world(3);
    let (mut bumped, mut feathers, mut bald, mut dazed) = (false, 0, 0.0_f32, 0.0_f32);
    let done = goose_a_play(&mut world, Stunt::WallSlam, 16.0, |w| {
        bumped |= w.sim.bumped.is_some();
        feathers = feathers.max(goose_a_feathers(w));
        bald = bald.max(w.sim.cond.bald);
        dazed = dazed.max(w.sim.cond.dazed);
    });
    assert!(done && bumped);
    assert!(feathers >= 2, "{feathers}");
    assert!(bald > 0.99 && dazed > 0.99, "{bald} {dazed}");
}

#[test]
fn goose_a_wall_slam_refuses_without_room_to_run() {
    let mut world = World::new(&config());
    world.sim.command(Command::Spawn(Some(Vec2::new(40.0, 600.0))));
    assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Spawned)));
    assert!(!world.sim.stage(Stunt::WallSlam));
}

#[test]
fn goose_a_honk_rings_and_sometimes_recoils() {
    let (mut recoils, mut plain) = (0, 0);
    for seed in 0..40 {
        let mut world = goose_a_world(seed);
        let (mut rings, mut tail) = (false, false);
        let done = goose_a_play(&mut world, Stunt::Honk, 12.0, |w| {
            rings |= goose_a_rings(w);
            tail |= w.sim.antic.pitch > 0.7;
        });
        assert!(done && rings, "{seed}");
        if tail {
            recoils += 1;
        } else {
            plain += 1;
        }
    }
    assert!(recoils > 0 && plain > recoils, "{recoils}/{plain}");
}

#[test]
fn goose_a_hiss_needs_a_cursor_nearby() {
    let mut world = World::alive(&config());
    assert!(!world.sim.stage(Stunt::Hiss));
    world.cursor = Some(world.sim.position() + Vec2::new(900.0, 0.0));
    world.tick();
    assert!(!world.sim.stage(Stunt::Hiss));
    assert!(!world.moves_cursor(3.0));
}

#[test]
fn goose_a_hiss_spreads_wings_and_lunges_at_a_close_cursor() {
    let mut world = World::alive(&config());
    world.run(1.0);
    let start = world.sim.position();
    world.cursor = Some(start + Vec2::new(60.0, 0.0));
    world.tick();
    let (mut wings, mut mouth, mut far) = (0.0_f32, 0.0_f32, 0.0_f32);
    let done = goose_a_play(&mut world, Stunt::Hiss, 9.0, |w| {
        wings = wings.max(w.sim.antic.wings);
        mouth = mouth.max(w.sim.antic.mouth);
        far = far.max(w.sim.position().distance(start));
    });
    assert!(done);
    assert_eq!((wings, mouth), (1.0, 1.0));
    assert!(far > 15.0, "{far}");
}

#[test]
fn goose_a_hiss_ends_when_the_cursor_leaves() {
    let mut world = World::alive(&config());
    world.run(1.0);
    world.cursor = Some(world.sim.position() + Vec2::new(150.0, 0.0));
    world.tick();
    world.sim.command(Command::Stage(Stunt::Hiss));
    world.run(1.0);
    assert!(!any_finished(&world));
    world.cursor = Some(world.sim.position() + Vec2::new(700.0, 0.0));
    assert!(world.run_until(3.0, finished(Stunt::Hiss)));
}

#[test]
fn goose_a_march_crosses_the_screen_head_high() {
    let mut world = goose_a_world(5);
    let start = world.sim.position();
    let mut neck = 0.0_f32;
    let done = goose_a_play(&mut world, Stunt::March, 14.0, |w| {
        neck = neck.max(w.sim.antic.neck_up);
    });
    assert!(done);
    assert_eq!(neck, 6.0);
    assert!(world.sim.position().distance(start) > 250.0);
}

#[test]
fn goose_a_skid_overshoots_in_dust_then_walks_back() {
    let mut world = goose_a_world(5);
    let (mut dust, mut lean) = (false, 0.0_f32);
    let done = goose_a_play(&mut world, Stunt::Skid, 14.0, |w| {
        dust |= !w.sim.particles().is_empty();
        lean = lean.max(w.sim.antic.pitch);
    });
    assert!(done && dust);
    assert!(lean >= 0.35, "{lean}");
}

#[test]
fn goose_a_flight_has_all_three_outcomes() {
    let (mut soared, mut flopped, mut knocked) = (0, 0, 0);
    for seed in 0..60 {
        let mut world = goose_a_world(seed);
        let (mut lift, mut wings, mut dazed) = (0.0_f32, 0.0_f32, 0.0_f32);
        let done = goose_a_play(&mut world, Stunt::Flight, 16.0, |w| {
            lift = lift.max(w.sim.antic.lift);
            wings = wings.max(w.sim.antic.wings);
            dazed = dazed.max(w.sim.cond.dazed);
        });
        assert!(done && wings == 1.0, "{seed}");
        if dazed > 0.0 {
            assert!(goose_a_feathers(&world) >= 2);
            knocked += 1;
        } else if lift > 40.0 {
            soared += 1;
        } else {
            assert!(lift <= 2.5, "{lift}");
            flopped += 1;
        }
    }
    assert!(soared > 0 && flopped > 0 && knocked > 0, "{soared}/{flopped}/{knocked}");
}

#[test]
fn goose_a_preen_drops_feathers_and_sometimes_overdoes_it() {
    let (mut bald, mut tidy) = (0, 0);
    for seed in 0..40 {
        let mut world = goose_a_world(seed);
        let (mut feathers, mut overdone) = (0, false);
        let done = goose_a_play(&mut world, Stunt::Preen, 12.0, |w| {
            feathers = feathers.max(goose_a_feathers(w));
            overdone |= w.sim.cond.bald > 0.0;
        });
        assert!(done && feathers >= 1, "{seed}");
        if overdone {
            bald += 1;
        } else {
            tidy += 1;
        }
    }
    assert!(bald > 0 && tidy > bald, "{bald}/{tidy}");
}
