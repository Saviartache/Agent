// Сценарии сценок свинки, группа B: файл подключается только к свинке.
use crate::effects::props::PropKind;

fn pig_b_world() -> World {
    let mut world = World::alive(&AgentConfig {
        start: Some(Vec2::new(900.0, 540.0)),
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

/// Как `run_until`, но наблюдатель может копить состояние.
fn pig_b_watch(world: &mut World, secs: f32, mut done: impl FnMut(&World) -> bool) -> bool {
    (0..(secs / DT) as usize).any(|_| {
        world.tick();
        done(world)
    })
}

fn pig_b_count(world: &World, kind: PropKind) -> usize {
    world.sim.props.iter().filter(|p| p.kind == kind).count()
}

#[test]
fn pig_b_dig_leaves_pits_and_eats_the_acorn() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::Dig));
    let (mut acorns, mut pits, mut moved) = (0, 0, false);
    let done = pig_b_watch(&mut world, 14.0, |w| {
        acorns = acorns.max(pig_b_count(w, PropKind::Acorn));
        pits = pits.max(w.sim.mud.iter().count());
        moved |= w.sim.antic.mouth > 0.0;
        finished(Stunt::Dig)(w)
    });
    assert!(done);
    assert_eq!(acorns, 1);
    assert!(pits >= 3, "{pits}");
    assert!(moved, "хруст");
    assert_eq!(pig_b_count(&world, PropKind::Acorn), 0);
    assert!(world.sim.cond.snout > 0.3);
}

#[test]
fn pig_b_sniff_refuses_without_a_cursor_path() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::Sniff));
    assert!(!world.run_until(3.0, finished(Stunt::Sniff)));
    assert!(world.sim.act.is_none());
}

#[test]
fn pig_b_sniff_follows_the_scent_to_the_cursor_without_moving_it() {
    let mut world = pig_b_world();
    let from = Vec2::new(1050.0, 540.0);
    for i in 0..240 {
        world.cursor = Some(from + Vec2::new(i as f32 * 0.6, 0.0));
        world.tick();
    }
    let at = world.cursor.unwrap();
    world.sim.command(Command::Stage(Stunt::Sniff));
    let mut snorted = false;
    let mut cursor_moved = false;
    for _ in 0..(16.0 / DT) as usize {
        cursor_moved |= world.tick().is_some();
        snorted |= world.sim.antic.eyes == crate::rig::Eyes::Wide;
        if world.has(&AgentEvent::StuntFinished(Stunt::Sniff)) {
            break;
        }
    }
    assert!(world.has(&AgentEvent::StuntFinished(Stunt::Sniff)));
    assert!(snorted);
    assert!(!cursor_moved);
    assert!(world.sim.position().distance(at) < 60.0);
}

#[test]
fn pig_b_snack_eats_the_apple_in_four_bites() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::Snack));
    let mut bites = 0;
    let done = pig_b_watch(&mut world, 20.0, |w| {
        let stage = w.sim.props.iter().map(|p| p.stage).max().unwrap_or(0);
        bites = bites.max(stage);
        finished(Stunt::Snack)(w)
    });
    assert!(done);
    assert_eq!(bites, 3);
    assert_eq!(pig_b_count(&world, PropKind::Apple), 0);
}

#[test]
fn pig_b_snack_overeating_bloats_and_deflates() {
    let mut rolled = false;
    for seed in 0..12 {
        let mut world = pig_b_world();
        world.sim.rng = crate::rng::Rng::new(seed);
        world.sim.command(Command::Stage(Stunt::Snack));
        let mut bloated = false;
        assert!(pig_b_watch(&mut world, 24.0, |w| {
            bloated |= w.sim.cond.bloated > 0.9 && w.sim.drive.is_some();
            finished(Stunt::Snack)(w)
        }));
        if bloated {
            rolled = true;
            assert!(world.sim.cond.bloated <= 0.2);
            assert!(world.sim.drive.is_none());
        }
    }
    assert!(rolled);
}

#[test]
fn pig_b_bulldozer_pushes_the_acorn_far_away() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::Bulldozer));
    world.tick();
    let start = world
        .sim
        .props
        .iter()
        .find(|p| p.kind == PropKind::Acorn)
        .unwrap()
        .pos;
    assert!(world.run_until(22.0, finished(Stunt::Bulldozer)));
    let end = world
        .sim
        .props
        .iter()
        .find(|p| p.kind == PropKind::Acorn)
        .unwrap()
        .pos;
    assert!(end.distance(start) > 350.0, "{}", end.distance(start));
}

#[test]
fn pig_b_tail_chase_spins_three_turns_then_dizzy() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::TailChase));
    let mut spin = 0.0_f32;
    let mut dazed = 0.0_f32;
    assert!(pig_b_watch(&mut world, 6.0, |w| {
        spin = spin.max(w.sim.antic.spin);
        dazed = dazed.max(w.sim.cond.dazed);
        finished(Stunt::TailChase)(w)
    }));
    assert!(spin >= 6.0 * std::f32::consts::PI - 0.01, "{spin}");
    assert!(dazed > 0.5);
}

#[test]
fn pig_b_piggy_bank_resolves_every_coin() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::PiggyBank));
    let mut coins = 0;
    let mut caught = false;
    assert!(pig_b_watch(&mut world, 12.0, |w| {
        let now = pig_b_count(w, PropKind::Coin);
        // Монета исчезла, не долежав, — поймана
        caught |= now < coins && w.sim.props.iter().all(|c| c.age < 4.9);
        coins = now;
        finished(Stunt::PiggyBank)(w)
    }));
    assert!(caught);
    // Пропущенные лежат на земле
    assert!(
        world
            .sim
            .props
            .iter()
            .all(|c| c.kind != PropKind::Coin || c.z <= 0.0 || c.vz > 0.0)
    );
}

#[test]
fn pig_b_belly_up_flips_onto_the_back_and_returns() {
    let mut world = pig_b_world();
    world.sim.command(Command::Stage(Stunt::BellyUp));
    let mut roll = 0.0_f32;
    let mut cursor_moved = false;
    for _ in 0..(8.0 / DT) as usize {
        cursor_moved |= world.tick().is_some();
        roll = roll.max(world.sim.antic.roll);
        if world.has(&AgentEvent::StuntFinished(Stunt::BellyUp)) {
            break;
        }
    }
    assert!(world.has(&AgentEvent::StuntFinished(Stunt::BellyUp)));
    assert!(roll >= std::f32::consts::PI - 0.01);
    assert!(!cursor_moved);
    assert_eq!(world.sim.antic.roll, 0.0);
}
