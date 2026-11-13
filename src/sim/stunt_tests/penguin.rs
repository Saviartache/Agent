// Сценарии сценок пингвина: файл подключается в модуль тела из `sim/tests.rs`.

/// Как `run_until`, но наблюдатель может копить состояние.
fn penguin_watch(world: &mut World, secs: f32, mut done: impl FnMut(&World) -> bool) -> bool {
    (0..(secs / DT) as usize).any(|_| {
        world.tick();
        done(world)
    })
}

fn penguin_pose(world: &World) -> Pose {
    match world.sim.character() {
        Some(CharacterView::Creature { pose, .. }) => pose.clone(),
        _ => unreachable!("пингвин без позы"),
    }
}

#[test]
fn penguin_toboggan_slides_on_the_belly_and_leaves_a_stripe() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Toboggan));
    let (mut prone, mut stripe) = (false, 0);
    let done = penguin_watch(&mut world, 12.0, |w| {
        prone |= w.sim.antic.pitch < -1.0 && w.sim.body.vel.length() > 100.0;
        stripe = stripe.max(w.sim.effects().scribble.iter().count());
        finished(Stunt::Toboggan)(w)
    });
    assert!(done && prone);
    assert!(stripe > 30, "{stripe}");
    assert!(world.run_until(8.0, |w| w.sim.effects().scribble.is_empty()));
}

#[test]
fn penguin_wipeout_falls_flat_and_gets_up_dazed() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Wipeout));
    let mut flat = false;
    let done = penguin_watch(&mut world, 14.0, |w| {
        flat |= w.sim.antic.pitch < -1.4;
        finished(Stunt::Wipeout)(w)
    });
    assert!(done && flat);
    assert!(world.sim.cond.dazed > 0.0);
}

#[test]
fn penguin_applause_claps_on_the_spot() {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world.run(1.0);
    let start = world.sim.position();
    world.sim.command(Command::Stage(Stunt::Applause));
    let (mut open, mut shut) = (false, false);
    let done = penguin_watch(&mut world, 6.0, |w| {
        let wings = penguin_pose(w).antic.wings;
        open |= wings > 0.9;
        shut |= open && wings < 0.1;
        finished(Stunt::Applause)(w)
    });
    assert!(done && open && shut);
    assert!(world.sim.position().distance(start) < 1.0);
}

#[test]
fn penguin_strut_crosses_the_screen_nodding() {
    let mut world = World::alive(&config());
    let start = world.sim.position();
    world.sim.command(Command::Stage(Stunt::Strut));
    let mut nod: f32 = 0.0;
    let done = penguin_watch(&mut world, 16.0, |w| {
        nod = nod.min(w.sim.antic.head_pitch);
        finished(Stunt::Strut)(w)
    });
    assert!(done && nod < -0.3);
    assert!((world.sim.position().x - start.x).abs() > 200.0);
}

#[test]
fn penguin_claps_flippers_on_success() {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world.run(1.0);
    assert!(penguin_pose(&world).antic.wings < 0.01);
    world.sim.command(Command::SetStatus(Status::Success));
    let mut most: f32 = 0.0;
    penguin_watch(&mut world, 2.0, |w| {
        most = most.max(penguin_pose(w).antic.wings);
        false
    });
    assert!(most > 0.5, "{most}");
}
