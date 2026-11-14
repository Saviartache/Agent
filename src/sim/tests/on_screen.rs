//! Сквозная проверка картинки: каждое действие и каждая сценка каждого вида идут через
//! настоящую симуляцию и рисуются тем же путём, что на рабочем столе, — тело, предметы,
//! частицы и гусята.

use super::*;
use crate::render::{Canvas, Figure, creature, scene};

const DT: f32 = 1.0 / 60.0;
const SPAWN_AT: Vec2 = Vec2::new(600.0, 600.0);
/// Курсор в стороне от персонажа: за ним можно идти, но он не под боком.
const FAR_CURSOR: Vec2 = Vec2::new(1000.0, 500.0);
/// Сценка обязана доиграть за это время.
const STUNT_SECS: f32 = 60.0;
/// Тело рисуется на каждом таком шаге, окно эффектов — на каждом таком.
const BODY_EVERY: usize = 12;
const EFFECTS_EVERY: usize = 60;

struct Stage {
    sim: Sim,
    events: Vec<AgentEvent>,
    cursor: Option<Vec2>,
    body: Figure,
    flock: Figure,
    pixels: Vec<u32>,
    /// Сколько шагов прошло и что на них было видно.
    steps: usize,
    body_seen: usize,
    effects_seen: usize,
}

impl Stage {
    fn alive(kind: RigKind) -> Self {
        let bounds = Bounds::new(vec![Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0)]).unwrap();
        let character = Character::builtin(kind).unwrap();
        // Шалости и захват курсора разрешены, иначе часть сценок не начинается;
        // сам персонаж сценок не затевает
        let config = AgentConfig {
            cursor_grab: CursorGrabConfig {
                enabled: true,
                ..CursorGrabConfig::default()
            },
            naughty: true,
            stunts: false,
            ..AgentConfig::default()
        };
        let mut stage = Self {
            sim: Sim::new(character, &config, bounds, 96, 7),
            events: Vec::new(),
            cursor: Some(FAR_CURSOR),
            body: Figure::default(),
            flock: Figure::default(),
            pixels: Vec::new(),
            steps: 0,
            body_seen: 0,
            effects_seen: 0,
        };
        stage.sim.command(Command::Spawn(Some(SPAWN_AT)));
        assert!(stage.run(6.0, |events| events.contains(&AgentEvent::Spawned)));
        stage.sim.command(Command::SetBehavior(BehaviorMode::Stay));
        stage.events.clear();
        stage
    }

    /// Идёт до `done` или `secs`; по пути рисует, как рисует поток персонажа.
    fn run(&mut self, secs: f32, done: impl Fn(&[AgentEvent]) -> bool) -> bool {
        for _ in 0..(secs / DT) as usize {
            let frame = Frame {
                dt: DT,
                cursor: self.cursor,
                user_input: false,
                permitted: true,
            };
            self.sim.step(&frame);
            self.sim.drain_events(&mut self.events);
            self.steps += 1;
            if self.steps.is_multiple_of(BODY_EVERY) {
                self.body_seen = self.body_seen.max(self.draw_body());
            }
            if self.steps.is_multiple_of(EFFECTS_EVERY) {
                self.effects_seen = self.effects_seen.max(self.draw_effects());
            }
            if done(&self.events) {
                return true;
            }
        }
        false
    }

    fn canvas(pixels: &mut Vec<u32>, rect: scene::PixelRect) -> Canvas<'_> {
        let (w, h) = (rect.width as usize, rect.height as usize);
        pixels.clear();
        pixels.resize(w * h, 0);
        Canvas::new(pixels, w, rect.width, rect.height).unwrap()
    }

    /// Непрозрачных пикселей в холсте тела.
    fn draw_body(&mut self) -> usize {
        let Some(CharacterView::Creature {
            pose,
            extent,
            skeleton,
        }) = self.sim.character()
        else {
            return 0;
        };
        let rect = creature::rect(pose, extent);
        let mut canvas = Self::canvas(&mut self.pixels, rect);
        let origin = Vec2::new(rect.x as f32, rect.y as f32);
        self.body.draw(&mut canvas, origin, pose, skeleton);
        self.pixels
            .iter()
            .filter(|pixel| **pixel >> 24 != 0)
            .count()
    }

    /// Непрозрачных пикселей в окне эффектов.
    fn draw_effects(&mut self) -> usize {
        let (effects, ui) = (self.sim.effects(), self.sim.ui_scale());
        let Some(area) = scene::effects_rect(&effects, ui) else {
            return 0;
        };
        let rect = scene::enclosing(area);
        let mut canvas = Self::canvas(&mut self.pixels, rect);
        let origin = Vec2::new(rect.x as f32, rect.y as f32);
        scene::draw_effects(&mut canvas, origin, &effects, ui, &mut self.flock);
        self.pixels
            .iter()
            .filter(|pixel| **pixel >> 24 != 0)
            .count()
    }
}

#[test]
fn muzzle_point_sits_on_the_drawn_muzzle() {
    for kind in RigKind::ALL {
        let mut stage = Stage::alive(kind);
        stage.run(1.0, |_| false);
        let Some(CharacterView::Creature { pose, extent, .. }) = stage.sim.character() else {
            panic!("{kind:?} без тела");
        };
        let rect = creature::rect(pose, extent);
        let head = stage.sim.head() - Vec2::new(rect.x as f32, rect.y as f32);
        // Точка у кончика морды: на нарисованном теле или в паре пикселей от него
        stage.draw_body();
        let (width, reach) = (rect.width as i32, 3);
        let near = (-reach..=reach).any(|dy| {
            (-reach..=reach).any(|dx| {
                let (x, y) = (head.x as i32 + dx, head.y as i32 + dy);
                let inside = x >= 0 && y >= 0 && x < width && y < rect.height as i32;
                inside && stage.pixels[(y * width + x) as usize] >> 24 != 0
            })
        });
        assert!(near, "{kind:?} {head:?}");
        // Земля под мордой ниже неё ровно на высоту
        let drop = stage.sim.muzzle_ground() - stage.sim.head();
        assert!(drop.x.abs() < 1e-3 && (drop.y - stage.sim.muzzle_height()).abs() < 1e-3);
        assert!(stage.sim.muzzle_height() > 0.0, "{kind:?}");
    }
}

#[test]
fn every_action_of_every_body_shows_on_screen() {
    for kind in RigKind::ALL {
        let names: Vec<String> = Character::builtin(kind)
            .unwrap()
            .action_names()
            .map(str::to_owned)
            .collect();
        assert!(names.len() >= 16, "{kind:?} {names:?}");
        for name in names {
            let mut stage = Stage::alive(kind);
            stage.sim.command(Command::Hold(Some(name.clone())));
            stage.run(1.5, |_| false);
            // Уход и появление сжимают тело, остальное — в полный рост
            let least = if matches!(name.as_str(), "spawn" | "despawn") {
                40
            } else {
                500
            };
            assert!(
                stage.body_seen > least,
                "{kind:?} {name} {}",
                stage.body_seen
            );
        }
    }
}

#[test]
fn every_stunt_of_every_body_plays_out_on_screen() {
    let (mut played, mut trouble) = (0, Vec::new());
    for kind in RigKind::ALL {
        for &stunt in Stunt::ALL {
            if stunt.cast().is_some_and(|cast| cast != kind) {
                continue;
            }
            let mut stage = Stage::alive(kind);
            let near = |stage: &Stage, by: f32| Some(stage.sim.position() + Vec2::new(by, 0.0));
            // Что сценке нужно, чтобы начаться
            match stunt {
                Stunt::FakeDeath => stage.cursor = near(&stage, 100.0),
                Stunt::Hiss => stage.cursor = near(&stage, 60.0),
                Stunt::MouseScare => stage.cursor = near(&stage, 120.0),
                Stunt::NeckTangle => stage.cursor = near(&stage, 200.0),
                Stunt::ShakeOff => stage.sim.cond.muddy = 1.0,
                Stunt::TripOverCard => {
                    let card = Notification::info("Карточка");
                    stage.sim.command(Command::Notify(card));
                    stage.run(5.0, |_| false);
                }
                _ => {}
            }
            // Курсор доходит до сценки со следующего шага
            stage.run(DT, |_| false);
            if !stage.sim.stage(stunt) {
                trouble.push(format!("{kind:?} не начал {stunt:?}"));
                continue;
            }
            let finished = AgentEvent::StuntFinished(stunt);
            let done = |events: &[AgentEvent]| events.contains(&finished);
            // «Мёртвым» лежит, пока курсор рядом: через несколько секунд он уходит
            let mut over = stunt == Stunt::FakeDeath && stage.run(4.0, done);
            if stunt == Stunt::FakeDeath && !over {
                stage.cursor = near(&stage, 600.0);
            }
            over = over || stage.run(STUNT_SECS, done);
            if !over {
                trouble.push(format!("{kind:?} не доиграл {stunt:?}"));
            }
            if stage.body_seen <= 300 {
                trouble.push(format!(
                    "{kind:?} не виден в {stunt:?}: {}",
                    stage.body_seen
                ));
            }
            played += 1;
        }
    }
    assert!(trouble.is_empty(), "{trouble:#?}");
    // Общие сценки — у каждого вида, свои — у своего
    let common = Stunt::ALL
        .iter()
        .filter(|stunt| stunt.cast().is_none())
        .count();
    let own = Stunt::ALL.len() - common;
    assert_eq!(played, common * RigKind::ALL.len() + own);
}

#[test]
fn props_particles_and_goslings_show_in_the_effects_window() {
    // Сценки, которые точно оставляют что-то в окне эффектов
    for (kind, stunt) in [
        (RigKind::Goose, Stunt::Goslings),
        (RigKind::Goose, Stunt::LeafPile),
        (RigKind::Pig, Stunt::MudBath),
        (RigKind::Pig, Stunt::Ball),
        (RigKind::Elephant, Stunt::Gift),
        (RigKind::Raccoon, Stunt::Butterfly),
        (RigKind::Penguin, Stunt::Sneeze),
    ] {
        let mut stage = Stage::alive(kind);
        stage.run(DT, |_| false);
        assert!(stage.sim.stage(stunt), "{kind:?} {stunt:?}");
        let finished = AgentEvent::StuntFinished(stunt);
        assert!(stage.run(STUNT_SECS, |events| events.contains(&finished)));
        assert!(
            stage.effects_seen > 30,
            "{kind:?} {stunt:?} {}",
            stage.effects_seen
        );
    }
}
