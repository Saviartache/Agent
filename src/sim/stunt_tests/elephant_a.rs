// Сценарии сценок слоника: файл подключается только к слонику.
use crate::effects::mud::Liquid;
use crate::effects::particles::Shape;

fn elephant_a_world() -> World {
    let mut world = World::alive(&AgentConfig {
        start: Some(Vec2::new(900.0, 540.0)),
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world
}

fn elephant_a_pose(world: &World) -> Pose {
    match world.sim.character() {
        Some(CharacterView::Creature { pose, .. }) => pose.clone(),
        _ => unreachable!("слоник без позы"),
    }
}

fn elephant_a_shapes(world: &World, shape: Shape) -> usize {
    world.sim.particles().iter().filter(|p| p.shape == shape).count()
}

/// Играет сценку до конца за `secs`; системный курсор не двигается.
fn elephant_a_play(world: &mut World, stunt: Stunt, secs: f32, mut look: impl FnMut(&World)) -> bool {
    world.sim.command(Command::Stage(stunt));
    for _ in 0..(secs / DT) as usize {
        assert!(world.tick().is_none(), "{stunt:?}: курсор тронут");
        look(world);
        if finished(stunt)(world) {
            return true;
        }
    }
    false
}

#[test]
fn elephant_a_trumpet_raises_the_trunk_with_notes_and_rings() {
    let mut world = elephant_a_world();
    let (mut raised, mut notes, mut rings) = (false, 0, 0);
    let done = elephant_a_play(&mut world, Stunt::Trumpet, 8.0, |w| {
        let pose = elephant_a_pose(w);
        raised |= pose.muzzle_tip().z > pose.head.center.z + pose.head.radius;
        notes = notes.max(elephant_a_shapes(w, Shape::Note));
        rings = rings.max(elephant_a_shapes(w, Shape::Ring));
    });
    assert!(done);
    assert!(raised, "хобот вверх");
    assert!(notes > 0 && rings > 0, "{notes} {rings}");
}

#[test]
fn elephant_a_shower_draws_water_with_the_trunk_and_pours_it_on_the_back() {
    let mut world = elephant_a_world();
    world.sim.command(Command::Stage(Stunt::Shower));
    world.tick();
    let puddle = world.sim.effects().mud.iter().next().unwrap();
    assert_eq!(puddle.liquid, Liquid::Water);
    let center = puddle.center;
    let (mut dipped, mut raised) = (false, false);
    let done = elephant_a_play(&mut world, Stunt::Shower, 20.0, |w| {
        let pose = elephant_a_pose(w);
        let tip = pose.muzzle_tip();
        let on_ground = pose.screen(Vec3::new(tip.x, tip.y, 0.0));
        dipped |= tip.z < 3.0 && on_ground.distance(center) < 30.0;
        raised |= tip.z > pose.head.center.z;
    });
    assert!(done);
    assert!(dipped, "кончик хобота в луже");
    assert!(raised, "хобот над спиной");
    world.run(4.0);
    assert!(world.sim.effects().mud.is_empty(), "лужу выпил и вылил");
}

#[test]
fn elephant_a_mouse_scare_refuses_without_a_cursor() {
    let mut world = elephant_a_world();
    world.sim.command(Command::Stage(Stunt::MouseScare));
    assert!(!world.run_until(3.0, finished(Stunt::MouseScare)));
    assert!(world.sim.act.is_none());
}

#[test]
fn elephant_a_mouse_scare_rears_up_and_backs_away_from_the_cursor() {
    let mut world = elephant_a_world();
    let start = world.sim.position();
    let mouse = start + Vec2::new(120.0, 0.0);
    world.cursor = Some(mouse);
    world.tick();
    let (mut wide, mut reared) = (false, false);
    let done = elephant_a_play(&mut world, Stunt::MouseScare, 8.0, |w| {
        wide |= w.sim.antic.eyes == crate::rig::Eyes::Wide;
        let pose = elephant_a_pose(w);
        reared |= pose.body.pitch > 0.4 && pose.fore.unwrap().iter().any(|f| f.z > 0.5);
    });
    assert!(done);
    assert!(wide && reared, "{wide} {reared}");
    assert!(world.sim.position().distance(mouse) > start.distance(mouse) + 10.0);
}

#[test]
fn elephant_a_trunk_nap_lays_the_trunk_down_and_snuffles() {
    let mut world = elephant_a_world();
    let (mut lying, mut puffs, mut zzz) = (false, 0, 0);
    let done = elephant_a_play(&mut world, Stunt::TrunkNap, 20.0, |w| {
        let pose = elephant_a_pose(w);
        let [_, tip] = pose.trunk.unwrap();
        lying |= pose.head.eyes_open < 0.01 && tip.z < 3.0;
        puffs = puffs.max(
            w.sim
                .particles()
                .iter()
                .filter(|p| [0xB8_AF_A0, 0xA8_9F_90].contains(&p.color))
                .count(),
        );
        zzz = zzz.max(elephant_a_shapes(w, Shape::Z));
    });
    assert!(done);
    assert!(lying, "хобот на земле, глаза закрыты");
    assert!(puffs > 0 && zzz > 0, "{puffs} {zzz}");
    assert!(world.sim.cond.sleepy > 0.9);
}
