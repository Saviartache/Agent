// Сценарии сценок (pig_a): файл подключается в модули тел из `sim/tests.rs`.

fn pig_a_world(naughty: bool) -> World {
    let mut world = World::alive(&AgentConfig {
        start: Some(Vec2::new(900.0, 540.0)),
        naughty,
        stunts: false,
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

fn pig_a_stage(world: &mut World, stunt: Stunt) {
    world.sim.command(Command::Stage(stunt));
    world.tick();
}

/// Сценка идёт; `watch` смотрит на мир каждый тик.
fn pig_a_play(world: &mut World, stunt: Stunt, secs: f32, mut watch: impl FnMut(&World)) -> bool {
    pig_a_stage(world, stunt);
    for _ in 0..(secs / DT) as usize {
        world.tick();
        watch(world);
        if world.has(&AgentEvent::StuntFinished(stunt)) {
            return true;
        }
    }
    false
}

fn pig_a_piles(world: &World) -> Vec<(Vec2, u8)> {
    world
        .sim
        .props
        .iter()
        .filter(|prop| prop.kind == crate::effects::props::PropKind::Pile)
        .map(|prop| (prop.pos, prop.stage))
        .collect()
}

#[test]
fn pig_a_oops_leaves_a_pile_that_flies_scatter_near_the_cursor() {
    let mut world = pig_a_world(true);
    let mut seen = None;
    assert!(pig_a_play(&mut world, Stunt::Oops, 20.0, |w| {
        seen = seen.or_else(|| pig_a_piles(w).first().copied());
    }));
    let (pile, _) = seen.expect("кучка");
    assert!(world.sim.position().distance(pile) > 20.0);
    assert_eq!(pig_a_piles(&world).len(), 1);
    let mut world = pig_a_world(true);
    pig_a_stage(&mut world, Stunt::Oops);
    assert!(world.run_until(20.0, |w| !pig_a_piles(w).is_empty()));
    let at = pig_a_piles(&world)[0].0;
    world.cursor = Some(at);
    world.tick();
    assert_eq!(pig_a_piles(&world)[0].1, 1);
    world.cursor = Some(at + Vec2::new(300.0, 0.0));
    world.tick();
    assert_eq!(pig_a_piles(&world)[0].1, 0);
}

#[test]
fn pig_a_naughty_stunts_refuse_without_permission() {
    for stunt in [Stunt::Oops, Stunt::FartJump] {
        let mut world = pig_a_world(false);
        world.sim.command(Command::Stage(stunt));
        world.run(1.0);
        assert!(world.sim.act.is_none(), "{stunt:?}");
        assert!(pig_a_piles(&world).is_empty());
    }
}

#[test]
fn pig_a_fart_jump_pops_the_pig_into_the_air_and_it_sniffs_the_cloud() {
    let mut world = pig_a_world(true);
    let mut high: f32 = 0.0;
    let mut turned: f32 = 0.0;
    assert!(pig_a_play(&mut world, Stunt::FartJump, 8.0, |w| {
        high = high.max(w.sim.antic.lift);
        turned = turned.max(w.sim.antic.spin);
    }));
    assert!(high > 15.0, "{high}");
    assert!(turned > 3.0, "{turned}");
}

#[test]
fn pig_a_faceplant_ploughs_a_furrow_and_dirties_the_snout() {
    let mut world = pig_a_world(false);
    let mut pitched = false;
    assert!(pig_a_play(&mut world, Stunt::Faceplant, 12.0, |w| {
        pitched |= w.sim.antic.pitch < -0.9;
    }));
    assert!(pitched);
    assert!(world.sim.cond.snout > 0.5, "{}", world.sim.cond.snout);
    assert!(world.sim.mud_prints.iter().count() >= 5);
}

#[test]
fn pig_a_slide_spins_along_the_belly_and_lies_on_the_side() {
    let mut world = pig_a_world(false);
    let start = world.sim.position();
    let (mut spin, mut roll): (f32, f32) = (0.0, 0.0);
    assert!(pig_a_play(&mut world, Stunt::Slide, 12.0, |w| {
        spin = spin.max(w.sim.antic.spin);
        roll = roll.max(w.sim.antic.roll);
    }));
    assert!(spin > 5.0, "{spin}");
    assert!(roll > 1.4, "{roll}");
    assert!(world.sim.position().distance(start) > 150.0);
}

#[test]
fn pig_a_stuck_hides_behind_the_edge_then_pops_out_dazed() {
    let mut world = pig_a_world(false);
    let mut clipped = false;
    assert!(pig_a_play(&mut world, Stunt::Stuck, 16.0, |w| {
        clipped |= w.sim.character_clip().is_some();
    }));
    assert!(clipped);
    assert!(world.sim.cond.dazed > 0.3, "{}", world.sim.cond.dazed);
    assert!(screen().hull().contains(world.sim.position()));
}

#[test]
fn pig_a_stuck_pops_early_when_the_cursor_comes_close() {
    let mut world = pig_a_world(false);
    pig_a_stage(&mut world, Stunt::Stuck);
    assert!(world.run_until(12.0, |w| w.sim.character_clip().is_some()));
    world.run(0.3);
    world.cursor = Some(world.sim.position());
    assert!(world.run_until(2.0, |w| w.sim.cond.dazed > 0.0));
}

#[test]
fn pig_a_shake_off_refuses_a_clean_pig_and_halves_the_mud() {
    let mut world = pig_a_world(false);
    world.sim.command(Command::Stage(Stunt::ShakeOff));
    world.run(0.5);
    assert!(world.sim.act.is_none());
    world.sim.cond.muddy = 1.0;
    assert!(pig_a_play(&mut world, Stunt::ShakeOff, 4.0, |_| {}));
    assert!((world.sim.cond.muddy - 0.5).abs() < 0.05);
}

#[test]
fn pig_a_muddy_pig_shakes_itself_off_sooner_or_later() {
    let mut world = World::alive(&AgentConfig {
        stunts: true,
        ..config()
    });
    world.sim.cond.muddy = 1.0;
    assert!(world.run_until(40.0, finished(Stunt::ShakeOff)));
    let mut clean = World::alive(&AgentConfig {
        stunts: true,
        ..config()
    });
    clean.run(40.0);
    assert!(!clean.has(&AgentEvent::StuntFinished(Stunt::ShakeOff)));
}

#[test]
fn pig_a_mud_nap_soaks_the_pig_in_a_puddle() {
    let mut world = pig_a_world(false);
    pig_a_stage(&mut world, Stunt::MudNap);
    let puddle = world.sim.mud.iter().next().unwrap().center;
    assert!(world.run_until(16.0, finished(Stunt::MudNap)));
    assert!(world.sim.position().distance(puddle) < 40.0);
    assert!(world.sim.cond.muddy > 0.99);
    assert!(world.sim.cond.snout > 0.9);
}

/// Как на рабочем столе: свободная прогулка, сценки сами не затеваются.
fn pig_a_wander_world() -> World {
    World::alive(&AgentConfig {
        start: Some(Vec2::new(900.0, 540.0)),
        naughty: true,
        stunts: false,
        ..config()
    })
}

/// Пока поза `still`, тело не сходит с места.
fn pig_a_holds_still(stunt: Stunt, secs: f32, still: impl Fn(&World) -> bool) {
    let mut world = pig_a_wander_world();
    let mut anchor: Option<Vec2> = None;
    let mut held = 0;
    let done = pig_a_play(&mut world, stunt, secs, |w| {
        if !still(w) {
            anchor = None;
            return;
        }
        let at = *anchor.get_or_insert(w.sim.position());
        assert!(w.sim.position().distance(at) < 3.0, "{stunt:?} ушёл");
        held += 1;
    });
    assert!(done, "{stunt:?} не доиграна");
    assert!(held > 10, "{stunt:?}: {held}");
}

#[test]
fn pig_a_stunts_stand_still_while_wandering() {
    pig_a_holds_still(Stunt::MudNap, 20.0, |w| w.sim.antic.lids > 0.99);
    pig_a_holds_still(Stunt::Oops, 24.0, |w| w.sim.antic.crouch == 4.0);
}

#[test]
fn pig_a_puddle_hop_jumps_the_chain_and_gets_dirty() {
    let mut world = pig_a_world(false);
    pig_a_stage(&mut world, Stunt::PuddleHop);
    let puddles: Vec<Vec2> = world.sim.mud.iter().map(|p| p.center).collect();
    assert!((3..=4).contains(&puddles.len()), "{}", puddles.len());
    let mut high: f32 = 0.0;
    for _ in 0..(24.0 / DT) as usize {
        world.tick();
        high = high.max(world.sim.antic.lift);
        if world.has(&AgentEvent::StuntFinished(Stunt::PuddleHop)) {
            break;
        }
    }
    assert!(world.has(&AgentEvent::StuntFinished(Stunt::PuddleHop)));
    assert!(high > 10.0, "{high}");
    assert!(world.sim.cond.muddy > 0.1, "{}", world.sim.cond.muddy);
    let at = world.sim.position();
    assert!(puddles.iter().any(|p| p.distance(at) < 40.0));
}
