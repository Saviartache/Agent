// Сценарии сценок (common_a): файл подключается в модули тел из `sim/tests.rs`.

fn common_a_world() -> World {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

/// Прогоняет `secs` и сообщает, что сценка успела показать.
#[derive(Default)]
struct CommonAWatch {
    particles: bool,
    wide: bool,
    cursor_moved: bool,
    lift: f32,
    wobble: f32,
    mouth: f32,
}

fn common_a_watch(world: &mut World, secs: f32, until: impl Fn(&World) -> bool) -> CommonAWatch {
    let mut seen = CommonAWatch::default();
    for _ in 0..(secs / DT) as usize {
        seen.cursor_moved |= world.tick().is_some();
        seen.particles |= !world.sim.particles().is_empty();
        seen.wide |= world.sim.antic.eyes == crate::rig::Eyes::Wide;
        seen.lift = seen.lift.max(world.sim.antic.lift);
        seen.wobble = seen.wobble.max(world.sim.antic.wobble.abs());
        seen.mouth = seen.mouth.max(world.sim.antic.mouth);
        if until(world) {
            break;
        }
    }
    seen
}

fn common_a_play(world: &mut World, stunt: Stunt, secs: f32) -> CommonAWatch {
    // Курсор доходит до сценки со следующего шага
    world.tick();
    world.sim.command(Command::Stage(stunt));
    let seen = common_a_watch(world, secs, finished(stunt));
    assert!(world.has(&AgentEvent::StuntFinished(stunt)), "{stunt:?}");
    assert!(!seen.cursor_moved, "{stunt:?}");
    seen
}

#[test]
fn common_a_chase_refuses_without_cursor() {
    let mut world = common_a_world();
    assert!(!world.sim.stage(Stunt::Chase));
}

#[test]
fn common_a_chase_runs_after_cursor_brakes_in_dust_and_gives_up() {
    let mut world = common_a_world();
    world.cursor = Some(Vec2::new(1300.0, 700.0));
    let start = world.sim.position();
    let seen = common_a_play(&mut world, Stunt::Chase, 10.0);
    assert!(seen.particles);
    assert!(world.sim.position().distance(start) > 200.0);
}

#[test]
fn common_a_boop_refuses_without_cursor() {
    let mut world = common_a_world();
    assert!(!world.sim.stage(Stunt::Boop));
}

#[test]
fn common_a_boop_pokes_the_cursor_with_sparks_or_misses_dazed() {
    let mut world = common_a_world();
    world.cursor = Some(Vec2::new(900.0, 600.0));
    let seen = common_a_play(&mut world, Stunt::Boop, 12.0);
    assert!(seen.particles || world.sim.cond.dazed > 0.0);
    assert_eq!(world.cursor, Some(Vec2::new(900.0, 600.0)));
}

#[test]
fn common_a_startle_hops_on_the_spot_without_cursor() {
    let mut world = common_a_world();
    let seen = common_a_play(&mut world, Stunt::Startle, 3.0);
    assert!(seen.lift > 10.0 && seen.wide && seen.particles);
}

#[test]
fn common_a_startle_flings_away_from_the_cursor() {
    let mut world = common_a_world();
    let at = world.sim.position();
    world.cursor = Some(at + Vec2::new(60.0, 0.0));
    common_a_play(&mut world, Stunt::Startle, 3.0);
    assert!(world.sim.position().x < at.x - 30.0);
}

#[test]
fn common_a_startle_reflex_fires_on_a_fast_cursor_and_pauses() {
    let mut world = common_a_world();
    let mut starts = 0;
    for step in 0..(30.0 / DT) as usize {
        // Пилообразный пролёт 2400 px/с: быстрее порога рефлекса, медленнее порога мозга
        let around = Vec2::new((step as f32 * 40.0) % 260.0 - 130.0, 20.0);
        world.cursor = Some(world.sim.position() + around);
        world.tick();
        starts += world
            .events
            .iter()
            .filter(|e| **e == AgentEvent::StuntFinished(Stunt::Startle))
            .count();
        world.events.clear();
    }
    // Пауза рефлекса 20 с: за 30 с — два испуга, не больше
    assert!((1..=2).contains(&starts), "{starts}");
}

#[test]
fn common_a_a_sleeping_pet_ignores_the_cursor() {
    let mut world = common_a_world();
    world.sim.command(Command::SetBehavior(BehaviorMode::Sleep));
    world.run(2.0);
    let at = world.sim.position();
    for step in 0..(10.0 / DT) as usize {
        // Пролёт 4000 px/с над самым персонажем
        let around = Vec2::new((step as f32 * 70.0) % 260.0 - 130.0, 10.0);
        world.cursor = Some(at + around);
        world.tick();
        assert!(world.sim.act.is_none(), "{:?}", world.sim.act);
    }
    assert!(world.sim.position().distance(at) < 1.0);
    assert_eq!(world.sim.brain.intent(), crate::behavior::Intent::Nap);
}

#[test]
fn common_a_startle_reflex_ignores_a_slow_cursor() {
    let mut world = common_a_world();
    for step in 0..(5.0 / DT) as usize {
        let around = Vec2::from_angle(step as f32 * 0.02) * 100.0;
        world.cursor = Some(world.sim.position() + around);
        world.tick();
    }
    assert!(!world.has(&AgentEvent::StuntFinished(Stunt::Startle)));
    assert!(world.sim.act.is_none());
}

#[test]
fn common_a_sneeze_puffs_and_shakes_off_mud() {
    let mut world = common_a_world();
    world.sim.cond.muddy = 1.0;
    let seen = common_a_play(&mut world, Stunt::Sneeze, 5.0);
    assert!(seen.particles && seen.mouth > 0.5);
    assert_eq!(world.sim.cond.muddy, 0.0);
    assert!(!world.sim.effects().mud.is_empty());
}

#[test]
fn common_a_sneeze_keeps_clean_fur_clean() {
    let mut world = common_a_world();
    common_a_play(&mut world, Stunt::Sneeze, 5.0);
    assert!(world.sim.effects().mud.is_empty());
}

#[test]
fn common_a_hiccups_bounce_with_bubbles() {
    let mut world = common_a_world();
    let seen = common_a_play(&mut world, Stunt::Hiccups, 9.0);
    assert!(seen.lift > 12.0 && seen.wide && seen.particles);
}

#[test]
fn common_a_dance_sways_with_notes() {
    let mut world = common_a_world();
    let seen = common_a_play(&mut world, Stunt::Dance, 8.0);
    assert!(seen.wobble > 1.0 && seen.lift > 1.0 && seen.particles);
}

#[test]
fn common_a_stretch_bows_and_yawns() {
    let mut world = common_a_world();
    let seen = common_a_play(&mut world, Stunt::Stretch, 5.0);
    assert!(seen.mouth > 0.9);
}

#[test]
fn common_a_stare_goes_to_the_middle_and_turns_to_the_viewer() {
    let mut world = common_a_world();
    let centre = screen().hull().center();
    world.cursor = Some(Vec2::new(300.0, 300.0));
    let mut turned = 0.0f32;
    world.sim.command(Command::Stage(Stunt::Stare));
    for _ in 0..(25.0 / DT) as usize {
        world.tick();
        turned = turned.max(world.sim.antic.spin.abs());
    }
    assert!(world.has(&AgentEvent::StuntFinished(Stunt::Stare)));
    assert!(turned > 0.5);
    assert!(world.sim.position().distance(centre) < 400.0);
}

#[test]
fn common_a_nap_sleeps_snoozes_and_leaves_the_pet_sleepy() {
    let mut world = common_a_world();
    let seen = common_a_play(&mut world, Stunt::Nap, 25.0);
    assert!(seen.particles);
    assert!(world.sim.cond.sleepy > 0.9);
}

#[test]
fn common_a_nap_sleeps_through_the_cursor() {
    let mut world = common_a_world();
    world.sim.command(Command::Stage(Stunt::Nap));
    world.run(3.0);
    world.cursor = Some(world.sim.position() + Vec2::new(30.0, 0.0));
    let seen = common_a_watch(&mut world, 6.0, finished(Stunt::Nap));
    assert!(!world.has(&AgentEvent::StuntFinished(Stunt::Nap)));
    assert!(!seen.wide);
}

#[test]
fn common_a_self_scare_flees_and_comes_back() {
    let mut world = common_a_world();
    world.sim.command(Command::MoveTo(Vec2::new(1000.0, 600.0)));
    world.run(4.0);
    let seen = common_a_play(&mut world, Stunt::SelfScare, 15.0);
    assert!(seen.wide && seen.lift > 10.0);
}
