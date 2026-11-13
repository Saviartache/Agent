// Сценарии сценок (goose_b): файл подключается в модули тел из `sim/tests.rs`.

fn naughty() -> AgentConfig {
    AgentConfig {
        naughty: true,
        ..config()
    }
}

/// Как `run_until`, но наблюдатель может копить состояние.
fn goose_b_watch(world: &mut World, secs: f32, mut done: impl FnMut(&World) -> bool) -> bool {
    (0..(secs / DT) as usize).any(|_| {
        world.tick();
        done(world)
    })
}

fn props_of(world: &World, kind: crate::effects::props::PropKind) -> usize {
    world.sim.effects().props.iter().filter(|p| p.kind == kind).count()
}

#[test]
fn goose_b_crumbs_are_all_pecked_away() {
    use crate::effects::props::PropKind::Crumb;
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Crumbs));
    world.tick();
    assert!((6..=9).contains(&props_of(&world, Crumb)));
    assert!(world.run_until(22.0, finished(Stunt::Crumbs)));
    assert_eq!(props_of(&world, Crumb), 0);
}

#[test]
fn goose_b_bath_splashes_and_the_puddle_dries() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Bath));
    world.tick();
    assert!(!world.sim.effects().mud.is_empty());
    assert!(world.run_until(14.0, finished(Stunt::Bath)));
    assert!(world.run_until(6.0, |w| w.sim.effects().mud.is_empty()));
}

#[test]
fn goose_b_nest_lays_an_egg_that_vanishes() {
    use crate::effects::props::PropKind::Egg;
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Nest));
    world.tick();
    assert!(!world.sim.effects().leaves.is_empty());
    let mut seen = false;
    let done = goose_b_watch(&mut world, 30.0, |w| {
        seen |= props_of(w, Egg) > 0;
        finished(Stunt::Nest)(w)
    });
    assert!(done && seen);
    assert!(world.run_until(2.0, |w| props_of(w, Egg) == 0));
}

#[test]
fn goose_b_egg_surprise_needs_naughty() {
    let mut world = World::alive(&config());
    assert!(!world.sim.stage(Stunt::EggSurprise));
}

#[test]
fn goose_b_egg_surprise_cracks_the_egg_and_rolls_out_an_arrow() {
    use crate::effects::props::PropKind::{Arrow, Egg};
    let mut world = World::alive(&naughty());
    assert!(world.sim.stage(Stunt::EggSurprise));
    let (mut egg, mut cracked, mut arrow) = (false, 0, false);
    let done = goose_b_watch(&mut world, 14.0, |w| {
        egg |= props_of(w, Egg) > 0;
        cracked = cracked.max(
            w.sim
                .effects()
                .props
                .iter()
                .filter(|p| p.kind == Egg)
                .map(|p| p.stage)
                .max()
                .unwrap_or(0),
        );
        arrow |= props_of(w, Arrow) > 0;
        finished(Stunt::EggSurprise)(w)
    });
    assert!(done && egg && arrow);
    assert_eq!(cracked, 3);
    assert!(!world.moves_cursor(1.0));
}

#[test]
fn goose_b_scribble_leaves_a_line_and_it_fades() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Scribble));
    let mut most = 0;
    let done = goose_b_watch(&mut world, 22.0, |w| {
        most = most.max(w.sim.effects().scribble.iter().count());
        finished(Stunt::Scribble)(w)
    });
    assert!(done);
    assert!(most > 30, "{most}");
    assert!(world.run_until(8.0, |w| w.sim.effects().scribble.is_empty()));
}

#[test]
fn goose_b_scribble_marks_lie_under_the_beak() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Scribble));
    let ui = world.sim.ui_scale();
    let mut checked = 0;
    for _ in 0..(22.0 / DT) as usize {
        world.tick();
        let near = world
            .sim
            .effects()
            .scribble
            .iter()
            .map(|print| print.pos.distance(world.sim.head()))
            .fold(f32::MAX, f32::min);
        if near < f32::MAX {
            // Свежая метка рядом с клювом (по высоте клюв чуть над землёй)
            assert!(near < 40.0 * ui, "{near}");
            checked += 1;
        }
        if finished(Stunt::Scribble)(&world) {
            break;
        }
    }
    assert!(checked > 100);
}

#[test]
fn goose_b_neck_tangle_needs_a_cursor() {
    let mut world = World::alive(&config());
    world.tick();
    assert!(!world.sim.stage(Stunt::NeckTangle));
}

#[test]
fn goose_b_neck_tangle_knots_the_neck_and_pops() {
    let mut world = World::alive(&config());
    world.cursor = Some(world.sim.position() + Vec2::new(200.0, 0.0));
    world.tick();
    assert!(world.sim.stage(Stunt::NeckTangle));
    let mut knotted = false;
    let done = goose_b_watch(&mut world, 16.0, |w| {
        knotted |= w.sim.antic.knot > 0.9;
        finished(Stunt::NeckTangle)(w)
    });
    assert!(done && knotted);
    assert!(!world.moves_cursor(1.0));
}

#[test]
fn goose_b_goslings_follow_and_leave() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Goslings));
    let mut most = 0;
    let done = goose_b_watch(&mut world, 32.0, |w| {
        most = most.max(w.sim.flock.len());
        finished(Stunt::Goslings)(w)
    });
    assert!(done);
    assert_eq!(most, 3);
    assert!(world.sim.flock.is_empty());
}
