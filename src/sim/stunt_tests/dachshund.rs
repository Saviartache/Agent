// Сценарии сценок таксы: файл подключается только к таксе.

fn dachshund_world() -> World {
    let mut world = World::alive(&AgentConfig {
        stunts: false,
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

/// Сценка идёт; `watch` смотрит на мир каждый тик.
fn dachshund_play(world: &mut World, stunt: Stunt, secs: f32, mut watch: impl FnMut(&World)) -> bool {
    // Курсор доходит до симуляции с тиком
    world.tick();
    world.sim.command(Command::Stage(stunt));
    (0..(secs / DT) as usize).any(|_| {
        world.tick();
        watch(world);
        world.has(&AgentEvent::StuntFinished(stunt))
    })
}

fn dachshund_slipper(world: &World) -> Option<crate::effects::props::Prop> {
    world
        .sim
        .props
        .iter()
        .find(|p| p.kind == crate::effects::props::PropKind::Slipper)
        .copied()
}

#[test]
fn dachshund_slipper_is_fetched_shaken_and_brought_to_the_cursor() {
    let mut world = dachshund_world();
    let cursor = Vec2::new(500.0, 700.0);
    world.cursor = Some(cursor);
    let (mut carried, mut shaken, mut wagged) = (false, false, false);
    assert!(dachshund_play(&mut world, Stunt::Slipper, 25.0, |w| {
        let antic = w.sim.antic;
        shaken |= antic.head_yaw.abs() > 0.5;
        wagged |= antic.wag > 0.0;
        carried |= dachshund_slipper(w).is_some_and(|s| s.z > 0.0 && shaken);
    }));
    assert!(carried && shaken && wagged);
    let slipper = dachshund_slipper(&world).expect("тапок остался у курсора");
    assert!(slipper.pos.distance(cursor) < 90.0, "{:?}", slipper.pos);
}

#[test]
fn dachshund_burrow_digs_pits_dives_behind_the_edge_and_comes_out_dirty() {
    let mut world = dachshund_world();
    let (mut pits, mut clipped) = (0, false);
    assert!(dachshund_play(&mut world, Stunt::Burrow, 20.0, |w| {
        pits = pits.max(w.sim.mud.iter().count());
        clipped |= w.sim.character_clip().is_some();
    }));
    assert!(pits >= 2, "{pits}");
    assert!(clipped);
    assert!(world.sim.cond.snout > 0.3);
    assert!(screen().hull().contains(world.sim.position()));
}

#[test]
fn dachshund_u_turn_tries_both_ways_squashed_then_walks_off_the_edge() {
    let mut world = dachshund_world();
    let (mut left, mut right, mut squashed) = (false, false, false);
    let mut edge_gap = f32::MAX;
    assert!(dachshund_play(&mut world, Stunt::UTurn, 20.0, |w| {
        let antic = w.sim.antic;
        left |= antic.spin < -0.5;
        right |= antic.spin > 0.5;
        squashed |= antic.squash.x < 0.85;
        let x = w.sim.position().x;
        edge_gap = edge_gap.min(x.min(1920.0 - x));
    }));
    assert!(left && right && squashed);
    let x = world.sim.position().x;
    assert!(x.min(1920.0 - x) > edge_gap + 60.0, "{edge_gap} {x}");
}

#[test]
fn dachshund_yap_refuses_without_a_cursor() {
    let mut world = dachshund_world();
    assert!(!world.sim.stage(Stunt::Yap));
}

#[test]
fn dachshund_yap_barks_at_the_cursor_facing_it() {
    let mut world = dachshund_world();
    let cursor = SPAWN_AT + Vec2::new(250.0, -40.0);
    world.cursor = Some(cursor);
    let (mut yaps, mut open) = (0, false);
    assert!(dachshund_play(&mut world, Stunt::Yap, 15.0, |w| {
        let now = w.sim.antic.mouth > 0.5;
        yaps += usize::from(now && !open);
        open = now;
    }));
    assert!(yaps >= 4, "{yaps}");
    let Some(CharacterView::Creature { pose, .. }) = world.sim.character() else {
        unreachable!("такса без позы");
    };
    assert!(pose.body.forward.x > 0.9, "{:?}", pose.body.forward);
    assert!(world.sim.position().distance(cursor) < 150.0);
}
