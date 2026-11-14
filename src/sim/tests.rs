use std::time::Duration;

use super::*;
use crate::command::{BehaviorMode, Notification, Status, Stunt};
use crate::config::CursorGrabConfig;
use crate::effects::items::ItemPhase;
use crate::math::Rect;
use crate::rig::{Pose, RigKind};

/// Что показано сейчас: кадр спрайта или поза тела.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Sprite([u32; 4]),
    Creature(Box<Pose>),
}

fn shown(sim: &Sim) -> Shown {
    match sim.character().unwrap() {
        CharacterView::Sprite(view) => Shown::Sprite(view.sprite.rect),
        CharacterView::Creature { pose, .. } => Shown::Creature(Box::new(pose.clone())),
    }
}

fn shown_scale(sim: &Sim) -> f32 {
    match sim.character().unwrap() {
        CharacterView::Sprite(view) => view.scale,
        CharacterView::Creature { pose, .. } => pose.scale,
    }
}

mod on_screen;

mod pig {
    use super::*;

    fn character() -> Character {
        Character::builtin(RigKind::Pig).unwrap()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
    include!("stunt_tests/pig_a.rs");
    include!("stunt_tests/pig_b.rs");

    #[test]
    fn four_feet_leave_prints_that_never_overlap() {
        let clear = 2.0 * RigKind::Pig.print_radius() * 0.99;
        for target in [
            Vec2::new(1500.0, 540.0),
            Vec2::new(900.0, 950.0),
            Vec2::new(300.0, 200.0),
        ] {
            let mut world = World::alive(&AgentConfig {
                start: Some(Vec2::new(900.0, 540.0)),
                ..config()
            });
            world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
            world.sim.command(Command::MoveTo(target));
            let mut seen = 0;
            for _ in 0..360 {
                world.tick();
                let prints: Vec<Vec2> = world.sim.footprints().iter().map(|p| p.pos).collect();
                seen = seen.max(prints.len());
                if let Some((last, rest)) = prints.split_last() {
                    assert!(
                        rest.iter().all(|p| p.distance(*last) >= clear),
                        "{target:?}"
                    );
                }
            }
            assert!(seen >= 12, "{seen}");
        }
    }

    fn pose_mud(world: &World) -> f32 {
        match world.sim.character() {
            Some(CharacterView::Creature { pose, .. }) => pose.look.mud,
            _ => unreachable!("свинка без позы"),
        }
    }

    #[test]
    fn mud_bath_makes_the_pig_muddy_and_leaves_a_long_fading_trail() {
        let mut world = World::alive(&AgentConfig {
            start: Some(Vec2::new(900.0, 540.0)),
            ..config()
        });
        world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
        world.sim.command(Command::Stage(Stunt::MudBath));
        world.tick();
        let puddle = world.sim.effects().mud.iter().next().unwrap().center;
        assert!(puddle.distance(world.sim.position()) > 100.0);
        assert!(world.run_until(16.0, finished(Stunt::MudBath)));
        assert!(world.sim.position().distance(puddle) < 30.0);
        // Обсыхает и стоя, но медленно
        assert!(world.sim.cond.muddy > 0.99, "{}", world.sim.cond.muddy);
        world.tick();
        assert!(pose_mud(&world) > 0.99);
        assert!(
            !world
                .events
                .iter()
                .any(|e| matches!(e, AgentEvent::Played(_)))
        );
        // Лужа высыхает, свинка уходит грязной
        world.run(4.0);
        assert!(world.sim.effects().mud.is_empty());
        let clean_prints = world.sim.footprints().iter().count();
        world.sim.command(Command::MoveTo(Vec2::new(200.0, 540.0)));
        world.run(6.0);
        let trail: Vec<f32> = world
            .sim
            .effects()
            .mud_prints
            .iter()
            .map(|p| p.opacity)
            .collect();
        assert!(trail.len() >= 30, "{}", trail.len());
        // Жирная от лужи и бледнеет к свинке
        let boldest = trail.iter().copied().fold(0.0, f32::max);
        assert!(boldest > 0.9 && trail[trail.len() - 1] < 0.6, "{trail:?}");
        assert!(
            world.sim.footprints().iter().count() <= clean_prints,
            "вместо обычных"
        );
        assert!(pose_mud(&world) < 1.0);
        // Грязная дорожка держится дольше обычных следов
        world.run(6.0);
        assert!(world.sim.footprints().is_empty());
        assert!(!world.sim.effects().mud_prints.is_empty());
        // Грязь стёрлась — снова обычные следы
        world.sim.command(Command::MoveTo(Vec2::new(1700.0, 540.0)));
        let clean = |w: &World| w.sim.cond.muddy == 0.0 && !w.sim.footprints().is_empty();
        assert!(world.run_until(16.0, clean));
    }

    #[test]
    fn respawned_pig_is_clean() {
        let mut world = World::alive(&config());
        world.sim.command(Command::Stage(Stunt::MudBath));
        assert!(world.run_until(16.0, finished(Stunt::MudBath)));
        world.sim.command(Command::Despawn);
        assert!(world.run_until(4.0, |w| w.has(&AgentEvent::Despawned)));
        world.sim.command(Command::Spawn(None));
        world.run(1.0);
        assert_eq!(pose_mud(&world), 0.0);
    }
}

mod goose {
    use super::*;

    fn character() -> Character {
        Character::builtin(RigKind::Goose).unwrap()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
    include!("stunt_tests/goose_a.rs");
    include!("stunt_tests/goose_b.rs");

    #[test]
    fn walking_leaves_footprints_where_the_feet_land() {
        let mut world = World::alive(&AgentConfig {
            start: Some(Vec2::new(200.0, 540.0)),
            ..config()
        });
        world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
        world.sim.command(Command::MoveTo(Vec2::new(1200.0, 540.0)));
        let mut checked = 0;
        let mut last = None;
        for _ in 0..600 {
            world.tick();
            let Some(print) = world.sim.footprints().iter().last() else {
                continue;
            };
            if last == Some(print.pos) {
                continue;
            }
            last = Some(print.pos);
            let Some(CharacterView::Creature { pose, .. }) = world.sim.character() else {
                unreachable!("гусь без позы");
            };
            let foot = &pose.feet[if print.left { 0 } else { 1 }];
            assert!(pose.screen(*foot).distance(print.pos) < 1e-3);
            checked += 1;
        }
        assert!(checked >= 6, "{checked}");
    }

    #[test]
    fn carried_card_follows_the_beak() {
        let mut world = World::alive(&config());
        world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
        world
            .sim
            .command(Command::Notify(Notification::info("Привет")));
        for _ in 0..20 {
            world.tick();
            let item = world.sim.items().iter().next().unwrap();
            assert_eq!(item.phase(), ItemPhase::Carried);
            let beak = world.sim.head();
            assert!(
                item.pos().distance(beak) < 40.0,
                "{:?} {beak:?}",
                item.pos()
            );
            assert!(item.pos().y < world.sim.position().y);
        }
    }

    #[test]
    fn dragging_hangs_the_goose_without_footprints() {
        let mut world = World::alive(&config());
        let start = world.sim.position();
        world.cursor = Some(start);
        world.sim.pointer(Pointer::Down(start));
        world.cursor = Some(Vec2::new(1400.0, 300.0));
        world.run(0.3);
        let before = world.sim.footprints().iter().count();
        world.run(1.0);
        assert!(world.sim.footprints().iter().count() <= before);
        let Some(CharacterView::Creature { pose, .. }) = world.sim.character() else {
            unreachable!("гусь без позы");
        };
        assert!(pose.feet.iter().all(|foot| foot.z > 0.0));
    }

    fn pile_center(world: &World) -> Vec2 {
        world.sim.effects().leaves.bounds().unwrap().center()
    }

    #[test]
    fn leaf_pile_is_run_through_and_scattered() {
        let mut world = World::alive(&config());
        world.sim.command(Command::Stage(Stunt::LeafPile));
        world.tick();
        let pile = pile_center(&world);
        let heaped = world.sim.effects().leaves.count_within(pile, 40.0);
        assert!(heaped > 30, "{heaped}");
        assert!(pile.distance(world.sim.position()) > 100.0);
        assert!(world.run_until(14.0, finished(Stunt::LeafPile)));
        assert!(world.sim.effects().leaves.count_within(pile, 40.0) < heaped / 2);
        // Радость после сценки — её часть, а не заказ приложения
        world.run(6.0);
        assert!(
            !world
                .events
                .iter()
                .any(|e| matches!(e, AgentEvent::Played(_)))
        );
        world.run(10.0);
        assert!(world.sim.effects().leaves.is_empty());
    }

    #[test]
    fn leaf_pile_is_played_while_busy_and_work_resumes() {
        let mut world = World::alive(&config());
        world.sim.command(Command::SetStatus(Status::Busy));
        world.run(1.0);
        world.sim.command(Command::Stage(Stunt::LeafPile));
        assert!(world.run_until(14.0, finished(Stunt::LeafPile)));
        world.run(4.0);
        assert!(world.travelled(3.0) < 1.0, "после сценки снова за работой");
    }

    #[test]
    fn second_leaf_pile_is_not_heaped_while_the_first_lies() {
        let mut world = World::alive(&config());
        world.sim.command(Command::Stage(Stunt::LeafPile));
        world.sim.command(Command::Stage(Stunt::LeafPile));
        world.tick();
        assert!(world.sim.effects().leaves.iter().count() <= 60);
    }
}

mod elephant {
    use super::*;
    use crate::rig::Vec3;

    fn character() -> Character {
        Character::builtin(RigKind::Elephant).unwrap()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
    include!("stunt_tests/elephant_a.rs");
}

mod raccoon {
    use super::*;

    fn character() -> Character {
        Character::builtin(RigKind::Raccoon).unwrap()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
    include!("stunt_tests/raccoon.rs");
}

mod penguin {
    use super::*;

    fn character() -> Character {
        Character::builtin(RigKind::Penguin).unwrap()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
    include!("stunt_tests/penguin.rs");
}

mod sprite {
    use super::*;

    fn character() -> Character {
        crate::assets::test_support::sprite_character()
    }

    include!("scenarios.rs");
    include!("stunt_tests/common_a.rs");
    include!("stunt_tests/common_b.rs");
}
