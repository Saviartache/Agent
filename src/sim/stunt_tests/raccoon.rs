// Сценарии сценок енота: файл подключается только к еноту.
use crate::effects::mud::Liquid;
use crate::effects::props::PropKind;

fn raccoon_world() -> World {
    let mut world = World::alive(&AgentConfig {
        start: Some(Vec2::new(900.0, 540.0)),
        stunts: false,
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

/// Как `run_until`, но наблюдатель может копить состояние.
fn raccoon_watch(world: &mut World, secs: f32, mut done: impl FnMut(&World) -> bool) -> bool {
    (0..(secs / DT) as usize).any(|_| {
        world.tick();
        done(world)
    })
}

fn raccoon_count(world: &World, kind: PropKind) -> usize {
    world.sim.props.iter().filter(|p| p.kind == kind).count()
}

#[test]
fn raccoon_rinse_washes_the_apple_in_water_and_eats_it() {
    let mut world = raccoon_world();
    world.sim.command(Command::Stage(Stunt::Rinse));
    world.tick();
    let puddle = world.sim.mud.iter().next().unwrap();
    assert_eq!(puddle.liquid, Liquid::Water);
    assert!(puddle.center.distance(world.sim.position()) > 100.0);
    let (mut wet, mut bites) = (false, 0);
    let done = raccoon_watch(&mut world, 20.0, |w| {
        let apple = w.sim.props.iter().find(|p| p.kind == PropKind::Apple);
        wet |= apple.is_some_and(|a| a.z == 0.0 && a.pos.distance(puddle.center) < 30.0);
        bites = bites.max(apple.map_or(0, |a| a.stage));
        finished(Stunt::Rinse)(w)
    });
    assert!(done);
    assert!(wet, "яблоко в луже");
    assert!(bites >= 2, "{bites}");
    assert_eq!(raccoon_count(&world, PropKind::Apple), 0);
    world.run(4.0);
    assert!(world.sim.mud.is_empty());
}

#[test]
fn raccoon_rummage_scatters_papers_from_the_bin() {
    let mut world = raccoon_world();
    world.sim.command(Command::Stage(Stunt::Rummage));
    world.tick();
    let bin = world.sim.props.iter().next().unwrap().pos;
    assert!(bin.distance(world.sim.position()) > 80.0);
    assert!(world.run_until(20.0, finished(Stunt::Rummage)));
    let papers: Vec<Vec2> = world
        .sim
        .props
        .iter()
        .filter(|p| p.kind == PropKind::Paper)
        .map(|p| p.pos)
        .collect();
    assert_eq!(papers.len(), 6);
    assert!(papers.iter().any(|p| p.distance(bin) > 30.0), "разлетелись");
    world.run(10.0);
    assert!(world.sim.props.iter().next().is_none());
}

#[test]
fn raccoon_stash_gives_the_note_only_when_clicked() {
    let mut world = raccoon_world();
    world.sim.command(Command::Stage(Stunt::Stash));
    world.run(3.0);
    assert!(world.sim.items().is_empty());
    assert_eq!(raccoon_count(&world, PropKind::Paper), 1);
    let at = world.sim.position() - Vec2::new(0.0, 10.0);
    world.cursor = Some(at);
    world.sim.pointer(Pointer::Down(at));
    world.tick();
    world.sim.pointer(Pointer::Up(at));
    assert!(world.run_until(3.0, finished(Stunt::Stash)));
    assert_eq!(world.sim.items().iter().count(), 1);
    assert_eq!(raccoon_count(&world, PropKind::Paper), 0);
}

#[test]
fn raccoon_stash_keeps_the_note_without_a_click_and_needs_an_empty_screen() {
    let mut world = raccoon_world();
    world.sim.command(Command::Stage(Stunt::Stash));
    assert!(world.run_until(14.0, finished(Stunt::Stash)));
    assert!(world.sim.items().is_empty());
    assert_eq!(raccoon_count(&world, PropKind::Paper), 0);
    world
        .sim
        .command(Command::Notify(Notification::info("Привет")));
    world.tick();
    assert!(!world.sim.stage(Stunt::Stash));
}

#[test]
fn raccoon_lookout_rises_on_hind_legs_and_looks_both_ways() {
    let mut world = raccoon_world();
    world.sim.command(Command::Stage(Stunt::Lookout));
    let at = world.sim.position();
    let (mut upright, mut left, mut right) = (0.0_f32, 0.0_f32, 0.0_f32);
    let done = raccoon_watch(&mut world, 6.0, |w| {
        upright = upright.max(w.sim.antic.pitch);
        left = left.min(w.sim.antic.head_yaw);
        right = right.max(w.sim.antic.head_yaw);
        finished(Stunt::Lookout)(w)
    });
    assert!(done);
    assert!(upright > 1.0, "{upright}");
    assert!(left < -0.5 && right > 0.5, "{left} {right}");
    assert!(world.sim.position().distance(at) < 3.0);
}

#[test]
fn raccoon_prowls_lower_than_it_stands() {
    let mut world = raccoon_world();
    world.run(1.0);
    let height = |w: &World| match w.sim.character() {
        Some(CharacterView::Creature { pose, .. }) => pose.body.center.z,
        _ => unreachable!("енот без позы"),
    };
    let standing = height(&world);
    world.sim.command(Command::MoveTo(Vec2::new(1500.0, 540.0)));
    world.run(1.0);
    // В среднем за шаги: тело подпрыгивает в такт лапам
    let ticks = (1.0 / DT) as usize;
    let mean = (0..ticks)
        .map(|_| {
            world.tick();
            height(&world)
        })
        .sum::<f32>()
        / ticks as f32;
    assert!(mean < standing - 1.5, "{mean} {standing}");
}
