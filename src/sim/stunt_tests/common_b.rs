// Сценарии сценок (common_b): файл подключается в модули тел из `sim/tests.rs`.

mod common_b {
use super::*;
use crate::effects::props::PropKind;
use crate::rig::Eyes;

fn common_b_naughty() -> AgentConfig {
    AgentConfig {
        naughty: true,
        ..config()
    }
}

/// Играет сценку до конца за `secs`; системный курсор при этом не двигается.
fn common_b_play(
    world: &mut World,
    stunt: Stunt,
    secs: f32,
    mut watch: impl FnMut(&World),
) -> bool {
    assert!(world.sim.stage(stunt), "{stunt:?} отказала");
    for _ in 0..(secs / DT) as usize {
        assert!(world.tick().is_none(), "курсор тронут");
        watch(world);
        if finished(stunt)(world) {
            return true;
        }
    }
    false
}

fn common_b_has(world: &World, kind: PropKind) -> bool {
    world.sim.props.iter().any(|prop| prop.kind == kind)
}

fn common_b_gift_kinds() -> &'static [PropKind] {
    match character().rig() {
        Some(RigKind::Goose) => &[PropKind::Feather, PropKind::Pebble],
        Some(RigKind::Pig) => &[PropKind::Acorn],
        Some(RigKind::Raccoon) => &[PropKind::Coin],
        None => &[PropKind::Pebble],
    }
}

fn common_b_gift_item(world: &World) -> Option<crate::effects::props::Prop> {
    let kinds = common_b_gift_kinds();
    world
        .sim
        .props
        .iter()
        .find(|prop| kinds.contains(&prop.kind))
        .copied()
}

#[test]
fn common_b_gift_leaves_the_edge_and_brings_an_item_to_the_cursor() {
    let mut world = World::alive(&config());
    let cursor = Vec2::new(1000.0, 500.0);
    world.cursor = Some(cursor);
    let (mut hidden, mut carried) = (false, false);
    let done = common_b_play(&mut world, Stunt::Gift, 22.0, |w| {
        hidden |= w.sim.character_clip().is_some();
        carried |= common_b_gift_item(w).is_some_and(|item| {
            let head = w.sim.head();
            (character().rig().is_none() || item.z > 0.0) && item.pos.distance(Vec2::new(head.x, w.sim.position().y)) < 5.0
        });
    });
    assert!(done);
    assert!(hidden, "уходил за край");
    assert!(carried, "нёс у головы");
    let item = common_b_gift_item(&world).expect("предмет лежит");
    assert!(item.pos.distance(cursor) < 120.0, "{:?}", item.pos);
    world.run(2.0);
    assert!(common_b_gift_item(&world).is_some_and(|item| item.resting()));
    assert!(world.sim.character_clip().is_none());
}

#[test]
fn common_b_gift_without_cursor_goes_to_the_middle_and_the_item_expires() {
    let mut world = World::alive(&config());
    assert!(common_b_play(&mut world, Stunt::Gift, 22.0, |_| {}));
    let item = common_b_gift_item(&world).expect("предмет лежит");
    assert!(item.pos.distance(Vec2::new(960.0, 540.0)) < 150.0, "{:?}", item.pos);
    world.run(31.0);
    assert!(common_b_gift_item(&world).is_none());
}

#[test]
fn common_b_ball_is_kicked_three_times_and_burns_out() {
    let mut world = World::alive(&config());
    let mut kicks = 0;
    let mut fast = false;
    let done = common_b_play(&mut world, Stunt::Ball, 24.0, |w| {
        let speed = w
            .sim
            .props
            .iter()
            .find(|p| p.kind == PropKind::Ball)
            .map_or(0.0, |p| p.vel.length());
        // Пинок — рывок скорости вверх от небыстрой
        if speed > 350.0 && !fast {
            kicks += 1;
        }
        fast = speed > 350.0;
    });
    assert!(done);
    assert!(kicks >= 3, "{kicks}");
    assert!(common_b_has(&world, PropKind::Ball));
    world.run(11.0);
    assert!(!common_b_has(&world, PropKind::Ball));
}

#[test]
fn common_b_ball_is_pushed_by_a_cursor_sweeping_through_it() {
    let mut world = World::alive(&config());
    assert!(world.sim.stage(Stunt::Ball));
    let ball = world
        .sim
        .props
        .iter()
        .find(|p| p.kind == PropKind::Ball)
        .copied()
        .unwrap();
    let mut pushed = 0.0_f32;
    for i in 0..30 {
        world.cursor = Some(ball.pos + Vec2::new(-60.0 + 10.0 * i as f32, 0.0));
        world.tick();
        if let Some(b) = world.sim.props.get(ball.id()) {
            pushed = pushed.max(b.vel.length());
        }
    }
    assert!(pushed > 100.0, "{pushed}");
}

#[test]
fn common_b_slip_throws_the_peel_and_leaves_the_character_dazed() {
    let mut world = World::alive(&config());
    let (mut roll, mut peel_flew) = (0.0_f32, false);
    let done = common_b_play(&mut world, Stunt::Slip, 13.0, |w| {
        roll = roll.max(w.sim.antic.roll);
        peel_flew |= w
            .sim
            .props
            .iter()
            .any(|p| p.kind == PropKind::Peel && p.z > 10.0);
    });
    assert!(done);
    assert!(roll > 3.0, "{roll}");
    assert!(peel_flew);
    assert!(world.sim.cond.dazed > 0.8);
}

#[test]
fn common_b_trip_over_card_needs_a_card_and_leaves_it_alone() {
    let mut world = World::alive(&config());
    assert!(!world.sim.stage(Stunt::TripOverCard));
    world
        .sim
        .command(Command::Notify(Notification::info("Карточка")));
    assert!(world.run_until(5.0, |w| w.first_item_phase() == Some(ItemPhase::Placed)));
    let card = world.sim.items().iter().next().unwrap().pos();
    let (mut pitch, mut dust) = (0.0_f32, false);
    let done = common_b_play(&mut world, Stunt::TripOverCard, 13.0, |w| {
        pitch = pitch.min(w.sim.antic.pitch);
        dust |= !w.sim.particles().is_empty();
        assert_eq!(w.first_item_phase(), Some(ItemPhase::Placed));
        assert_eq!(w.sim.items().iter().next().unwrap().pos(), card);
    });
    assert!(done);
    assert!(pitch < -0.8, "{pitch}");
    assert!(dust);
}

#[test]
fn common_b_peek_goes_beyond_the_edge_and_comes_back() {
    let mut world = World::alive(&config());
    let (mut hidden, mut outside) = (0, false);
    let done = common_b_play(&mut world, Stunt::Peek, 22.0, |w| {
        hidden += usize::from(w.sim.character_clip().is_some());
        outside |= !screen().contains(w.sim.position());
    });
    assert!(done);
    assert!(hidden > 120, "{hidden}");
    assert!(outside);
    world.run(0.5);
    assert!(screen().contains(world.sim.position()));
    assert!(world.sim.character_clip().is_none());
}

#[test]
fn common_b_peek_hides_from_a_cursor_that_follows_the_head() {
    let mut world = World::alive(&config());
    assert!(world.sim.stage(Stunt::Peek));
    let mut done = false;
    for _ in 0..(22.0 / DT) as usize {
        world.cursor = Some(world.sim.head());
        world.tick();
        done = finished(Stunt::Peek)(&world);
        if done {
            break;
        }
    }
    assert!(done);
}

#[test]
fn common_b_edge_tumble_falls_below_the_screen_and_climbs_back() {
    let mut world = World::alive(&config());
    let (mut hidden, mut below, mut roll, mut wide) = (false, false, 0.0_f32, false);
    let done = common_b_play(&mut world, Stunt::EdgeTumble, 13.0, |w| {
        hidden |= w.sim.character_clip().is_some();
        below |= w.sim.position().y > 1080.0;
        roll = roll.max(w.sim.antic.roll);
        wide |= w.sim.antic.eyes == Eyes::Wide;
    });
    assert!(done);
    assert!(hidden && below && wide);
    assert!(roll > 3.0, "{roll}");
    assert!(screen().contains(world.sim.position()));
}

#[test]
fn common_b_butterfly_flies_away_and_fades() {
    let mut world = World::alive(&config());
    let (mut airborne, mut neck) = (false, 0.0_f32);
    let done = common_b_play(&mut world, Stunt::Butterfly, 15.0, |w| {
        airborne |= w
            .sim
            .props
            .iter()
            .any(|p| p.kind == PropKind::Butterfly && p.z > 20.0);
        neck = neck.max(w.sim.antic.neck_up);
    });
    assert!(done);
    assert!(airborne);
    assert!(neck >= 1.0);
    world.run(1.0);
    assert!(!common_b_has(&world, PropKind::Butterfly));
}

#[test]
fn common_b_flatten_squashes_then_springs_back() {
    let mut world = World::alive(&config());
    let (mut flat, mut crossed) = (f32::MAX, false);
    let done = common_b_play(&mut world, Stunt::Flatten, 8.0, |w| {
        flat = flat.min(w.sim.antic.squash.y);
        crossed |= w.sim.antic.eyes == Eyes::Crossed;
    });
    assert!(done);
    assert!(flat < 0.3, "{flat}");
    assert!(crossed);
    assert!(world.sim.cond.dazed > 0.3);
    assert!(common_b_has(&world, PropKind::Bucket));
    world.run(5.0);
    assert!(!common_b_has(&world, PropKind::Bucket));
}

#[test]
fn common_b_fake_death_needs_naughty() {
    let mut world = World::alive(&config());
    assert!(!world.sim.stage(Stunt::FakeDeath));
}

#[test]
fn common_b_fake_death_lies_while_the_cursor_is_near_then_jumps_up() {
    let mut world = World::alive(&common_b_naughty());
    world.cursor = Some(world.sim.position() + Vec2::new(100.0, 0.0));
    world.tick();
    let mut tongue = false;
    assert!(world.sim.stage(Stunt::FakeDeath));
    for _ in 0..(8.0 / DT) as usize {
        assert!(world.tick().is_none());
        tongue |= world.sim.antic.tongue;
    }
    assert!(tongue);
    assert!(world.sim.antic.roll > 3.0, "лежит, пока курсор рядом");
    assert!(!any_finished(&world));
    world.cursor = Some(world.sim.position() + Vec2::new(600.0, 0.0));
    assert!(world.run_until(4.0, finished(Stunt::FakeDeath)));
}

#[test]
fn common_b_fake_death_without_a_cursor_lies_briefly() {
    let mut world = World::alive(&common_b_naughty());
    let mut roll = 0.0_f32;
    let done = common_b_play(&mut world, Stunt::FakeDeath, 10.0, |w| {
        roll = roll.max(w.sim.antic.roll);
    });
    assert!(done);
    assert!(roll > 3.0, "{roll}");
}
}
