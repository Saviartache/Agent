// Сценарии общие: файл подключается и к свинке, и к гусю, и к спрайтовому набору.
const DT: f32 = 1.0 / 60.0;
const SPAWN_AT: Vec2 = Vec2::new(600.0, 600.0);

fn screen() -> Bounds {
    Bounds::new(vec![Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0)]).unwrap()
}

struct World {
    sim: Sim,
    events: Vec<AgentEvent>,
    cursor: Option<Vec2>,
    /// Пользователь жмёт кнопку мыши или Esc.
    input: bool,
}

impl World {
    fn with(config: &AgentConfig, bounds: Bounds) -> Self {
        Self {
            sim: Sim::new(character(), config, bounds, 96, 7),
            events: Vec::new(),
            cursor: None,
            input: false,
        }
    }

    fn new(config: &AgentConfig) -> Self {
        Self::with(config, screen())
    }

    /// Появился и отыграл `spawn`.
    fn alive(config: &AgentConfig) -> Self {
        let mut world = Self::new(config);
        world.sim.command(Command::Spawn(Some(SPAWN_AT)));
        assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Spawned)));
        world.events.clear();
        world
    }

    fn tick(&mut self) -> Option<Vec2> {
        let frame = Frame {
            dt: DT,
            cursor: self.cursor,
            user_input: self.input,
            permitted: true,
        };
        let moved = self.sim.step(&frame);
        self.sim.drain_events(&mut self.events);
        if moved.is_some() {
            self.cursor = moved;
        }
        moved
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            self.tick();
        }
    }

    fn run_until(&mut self, secs: f32, done: impl Fn(&World) -> bool) -> bool {
        for _ in 0..(secs / DT) as usize {
            self.tick();
            if done(self) {
                return true;
            }
        }
        false
    }

    /// Персонаж двигал системный курсор хоть раз за `secs`.
    fn moves_cursor(&mut self, secs: f32) -> bool {
        (0..(secs / DT) as usize).fold(false, |moved, _| self.tick().is_some() || moved)
    }

    fn has(&self, event: &AgentEvent) -> bool {
        self.events.contains(event)
    }

    fn travelled(&mut self, secs: f32) -> f32 {
        let mut last = self.sim.position();
        let mut total = 0.0;
        for _ in 0..(secs / DT) as usize {
            self.tick();
            total += self.sim.position().distance(last);
            last = self.sim.position();
        }
        total
    }

    fn first_item(&self) -> crate::effects::items::ItemId {
        self.sim.items().iter().next().unwrap().id()
    }

    fn first_item_phase(&self) -> Option<ItemPhase> {
        self.sim.items().iter().next().map(|item| item.phase())
    }
}

fn config() -> AgentConfig {
    AgentConfig::default()
}

fn closed(by_user: bool) -> AgentEvent {
    AgentEvent::NotificationClosed { by_user }
}

#[test]
fn spawn_goes_to_bottom_centre_and_life_stays_on_screen() {
    let mut world = World::new(&config());
    assert!(world.sim.is_dormant());
    world.sim.command(Command::Spawn(None));
    assert_eq!(world.sim.position(), Vec2::new(960.0, 1080.0 - 56.0));
    assert!(!world.sim.is_dormant());
    assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Spawned)));
    let hull = screen().hull();
    let mut last = world.sim.position();
    let mut travelled = 0.0;
    for _ in 0..3600 {
        world.tick();
        let at = world.sim.position();
        assert_eq!(hull.clamp(at, 0.0), at);
        travelled += at.distance(last);
        last = at;
    }
    assert!(travelled > 100.0, "{travelled}");
}

#[test]
fn second_spawn_while_present_is_ignored() {
    let mut world = World::alive(&config());
    let at = world.sim.position();
    world
        .sim
        .command(Command::Spawn(Some(Vec2::new(100.0, 100.0))));
    world.run(0.5);
    assert!(world.sim.position().distance(at) < 50.0);
    assert!(!world.has(&AgentEvent::Spawned));
}

#[test]
fn huge_frame_moves_at_most_a_quarter_second() {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world.sim.command(Command::MoveTo(Vec2::new(1800.0, 900.0)));
    world.run(1.0);
    let before = world.sim.position();
    let frame = Frame {
        dt: 1e6,
        cursor: None,
        user_input: false,
        permitted: true,
    };
    world.sim.step(&frame);
    let run_speed = world.sim.character.motion().run_speed;
    assert!(world.sim.position().distance(before) <= run_speed * 0.25 + 1.0);
    for dt in [f32::NAN, -5.0, f32::NEG_INFINITY] {
        let before = world.sim.position();
        world.sim.step(&Frame { dt, ..frame });
        assert!(world.sim.position().distance(before) < 1.0, "{dt}");
    }
}

#[test]
fn not_permitted_freezes_the_simulation() {
    let mut world = World::alive(&config());
    let before = world.sim.position();
    let frame = Frame {
        dt: 0.2,
        cursor: None,
        user_input: false,
        permitted: false,
    };
    assert_eq!(world.sim.step(&frame), None);
    assert_eq!(world.sim.position(), before);
}

#[test]
fn pause_freezes_everything_until_resumed() {
    let mut world = World::alive(&config());
    world.run(3.0);
    world.sim.command(Command::Pause(true));
    assert!(world.sim.paused());
    let snapshot = (
        world.sim.position(),
        shown(&world.sim),
        world.sim.footprints().iter().count(),
    );
    world.run(5.0);
    let after = (
        world.sim.position(),
        shown(&world.sim),
        world.sim.footprints().iter().count(),
    );
    assert_eq!(snapshot, after);
    assert!(world.events.is_empty());
    world.sim.command(Command::Pause(false));
    assert!(world.travelled(10.0) > 10.0);
}

#[test]
fn play_reports_played_and_unknown_is_ignored() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Play("nonsense".into()));
    world.run(3.0);
    assert!(world.events.is_empty());
    world.sim.command(Command::Play("wave".into()));
    assert!(world.run_until(4.0, |w| w.has(&AgentEvent::Played("wave".into()))));
}

#[test]
fn busy_then_success_reacts_and_returns_to_wandering() {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetStatus(Status::Busy));
    world.run(2.0);
    assert!(world.travelled(3.0) < 1.0);
    world.sim.command(Command::SetStatus(Status::Success));
    assert!(world.run_until(2.0, |w| !w.sim.particles().is_empty()));
    world.run(5.0);
    assert!(world.travelled(40.0) > 20.0);
}

#[test]
fn notification_is_carried_placed_and_expires() {
    let mut world = World::alive(&config());
    let note = Notification::success("Готово").ttl(Duration::from_secs(1));
    world.sim.command(Command::Notify(note));
    assert_eq!(world.first_item_phase(), Some(ItemPhase::Carried));
    assert!(world.run_until(4.0, |w| w.first_item_phase() == Some(ItemPhase::Placed)));
    assert!(world.run_until(3.0, |w| w.has(&closed(false))));
    assert!(world.sim.items().is_empty());
    // Перенос — не приказ приложения
    assert!(
        !world
            .events
            .iter()
            .any(|e| matches!(e, AgentEvent::Played(_)))
    );
}

#[test]
fn dismissing_a_card_reports_the_user() {
    let mut world = World::alive(&config());
    world
        .sim
        .command(Command::Notify(Notification::info("Привет")));
    assert!(world.run_until(4.0, |w| w.first_item_phase() == Some(ItemPhase::Placed)));
    world.sim.dismiss_item(world.first_item());
    assert!(world.run_until(1.0, |w| w.has(&closed(true))));
}

#[test]
fn cards_stack_upward_and_overflow_closes_the_oldest() {
    let mut world = World::alive(&AgentConfig {
        max_items: 2,
        ..config()
    });
    for title in ["раз", "два", "три"] {
        world
            .sim
            .command(Command::Notify(Notification::info(title)));
        world.run(1.5);
    }
    assert_eq!(world.sim.items().len(), 2);
    assert!(world.has(&closed(false)));
    let ys: Vec<f32> = world.sim.items().iter().map(|i| i.pos().y).collect();
    assert!(ys[1] < ys[0], "{ys:?}");
}

#[test]
fn notification_without_character_is_placed_at_once() {
    let mut world = World::new(&config());
    world
        .sim
        .command(Command::Notify(Notification::warning("Нет персонажа")));
    assert!(!world.sim.is_dormant());
    world.run(1.0);
    let item = world.sim.items().iter().next().unwrap();
    assert!(matches!(
        item.phase(),
        ItemPhase::Settling | ItemPhase::Placed
    ));
    assert!(screen().contains(item.pos()) || item.pos().y == 1080.0 - 8.0);
}

#[test]
fn press_without_movement_is_a_click() {
    let mut world = World::alive(&config());
    let at = world.sim.position();
    world.cursor = Some(at);
    world.sim.pointer(Pointer::Down(at));
    world.run(0.2);
    world.sim.pointer(Pointer::Up(at));
    world.run(0.1);
    assert!(world.has(&AgentEvent::Clicked));
    assert!(
        !world
            .events
            .iter()
            .any(|e| matches!(e, AgentEvent::Dropped(_)))
    );
}

#[test]
fn press_and_move_drags_the_body_with_the_cursor() {
    let mut world = World::alive(&config());
    let start = world.sim.position();
    world.cursor = Some(start);
    world.sim.pointer(Pointer::Down(start));
    let target = Vec2::new(1000.0, 400.0);
    world.cursor = Some(target);
    world.run(0.5);
    assert!(world.sim.position().distance(target) < 1.0);
    world.sim.pointer(Pointer::Up(target));
    world.run(0.1);
    let dropped = world.events.iter().find_map(|e| match e {
        AgentEvent::Dropped(at) => Some(*at),
        _ => None,
    });
    assert!(dropped.is_some_and(|at| at.distance(target) < 1.0));
    assert!(!world.has(&AgentEvent::Clicked));
    assert!(!world.sim.particles().is_empty());
}

#[test]
fn cancelled_press_neither_clicks_nor_drops() {
    let mut world = World::alive(&config());
    let start = world.sim.position();
    world.cursor = Some(start);
    world.sim.pointer(Pointer::Down(start));
    world.cursor = Some(start + Vec2::new(200.0, 0.0));
    world.run(0.3);
    world.sim.pointer(Pointer::Cancel);
    world.sim.pointer(Pointer::Up(start));
    world.run(0.3);
    assert!(world.events.is_empty());
}

#[test]
fn undraggable_character_ignores_movement() {
    let mut world = World::alive(&AgentConfig {
        draggable: false,
        ..config()
    });
    let start = world.sim.position();
    world.cursor = Some(start);
    world.sim.pointer(Pointer::Down(start));
    let far = start + Vec2::new(300.0, 0.0);
    world.cursor = Some(far);
    world.run(0.5);
    assert!(world.sim.position().distance(far) > 100.0);
    world.sim.pointer(Pointer::Up(far));
    world.run(0.1);
    assert!(world.has(&AgentEvent::Clicked));
}

#[test]
fn pointer_on_absent_character_is_ignored() {
    let mut world = World::new(&config());
    world.sim.pointer(Pointer::Down(Vec2::new(10.0, 10.0)));
    world.sim.pointer(Pointer::Up(Vec2::new(10.0, 10.0)));
    world.run(0.1);
    assert!(world.events.is_empty());
}

#[test]
fn removed_monitor_relocates_body_and_cards() {
    let both = Bounds::new(vec![
        Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0),
        Rect::from_xywh(1920.0, 0.0, 1920.0, 1080.0),
    ])
    .unwrap();
    let mut world = World::with(
        &AgentConfig {
            start: Some(Vec2::new(3000.0, 600.0)),
            ..config()
        },
        both,
    );
    world.sim.command(Command::Spawn(None));
    assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Spawned)));
    world
        .sim
        .command(Command::Notify(Notification::info("Второй монитор")));
    assert!(world.run_until(4.0, |w| w.first_item_phase() == Some(ItemPhase::Placed)));
    assert!(world.sim.items().iter().next().unwrap().pos().x > 1920.0);

    world.sim.set_display(screen(), 96);
    assert!(world.sim.position().x <= 1920.0);
    assert!(world.sim.items().iter().all(|item| item.pos().x <= 1920.0));
}

#[test]
fn slightly_outside_body_is_left_to_walk_back() {
    let mut world = World::alive(&config());
    let before = world.sim.position();
    let narrow = Bounds::new(vec![Rect::from_xywh(0.0, 0.0, 550.0, 1080.0)]).unwrap();
    world.sim.set_display(narrow, 96);
    assert_eq!(world.sim.position(), before);
}

#[test]
fn dpi_change_rescales_the_character() {
    let mut world = World::alive(&config());
    let before = shown_scale(&world.sim);
    world.sim.set_display(screen(), 192);
    let after = shown_scale(&world.sim);
    assert!((after / before - 2.0).abs() < 1e-4);
    assert!((world.sim.ui_scale() - 2.0).abs() < 1e-4);
}

#[test]
fn footprints_follow_walking_and_fade_after_stopping() {
    let mut world = World::alive(&AgentConfig {
        start: Some(Vec2::new(200.0, 540.0)),
        ..config()
    });
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    let target = Vec2::new(1200.0, 540.0);
    world.sim.command(Command::MoveTo(target));
    assert!(world.run_until(8.0, |w| !w.sim.footprints().is_empty()));
    assert!(world.run_until(20.0, |w| w.sim.position().distance(target) < 10.0));
    assert!(world.run_until(9.0, |w| w.sim.footprints().is_empty()));
}

#[test]
fn disabled_footprints_leave_nothing() {
    let mut world = World::alive(&AgentConfig {
        footprints: false,
        ..config()
    });
    world.sim.command(Command::MoveTo(Vec2::new(1200.0, 540.0)));
    world.run(10.0);
    assert!(world.sim.footprints().is_empty());
}

#[test]
fn despawn_ends_in_dormancy_once_effects_expire() {
    let mut world = World::alive(&config());
    world.run(3.0);
    world.sim.command(Command::Despawn);
    assert!(world.run_until(6.0, |w| w.has(&AgentEvent::Despawned)));
    assert!(world.sim.character().is_none());
    assert!(world.run_until(10.0, |w| w.sim.is_dormant()));
}

/// Курсор не уходит от персонажа далеко и стоит на месте, пока его не двинет захват.
fn keep_cursor_near(world: &mut World) {
    let at = world.sim.position();
    if world.cursor.is_none_or(|c| c.distance(at) > 250.0) {
        world.cursor = Some(at + Vec2::new(100.0, 0.0));
    }
}

fn grab_run(config: &AgentConfig, secs: f32) -> usize {
    let mut world = World::alive(config);
    let mut grabs = 0;
    for _ in 0..(secs / DT) as usize {
        keep_cursor_near(&mut world);
        grabs += usize::from(world.tick().is_some());
    }
    grabs
}

#[test]
fn cursor_is_never_moved_unless_grab_is_enabled() {
    for seed in 0..8 {
        let off = AgentConfig {
            seed: Some(seed),
            ..config()
        };
        assert_eq!(grab_run(&off, 200.0), 0, "seed {seed}");
    }
}

#[test]
fn enabled_grab_moves_the_cursor_toward_the_character() {
    let on = |seed| AgentConfig {
        seed: Some(seed),
        cursor_grab: CursorGrabConfig {
            enabled: true,
            ..CursorGrabConfig::default()
        },
        ..config()
    };
    assert!((0..40).any(|seed| grab_run(&on(seed), 200.0) > 0));
}

#[test]
fn same_seed_same_inputs_same_life() {
    let script = |world: &mut World| {
        let mut path = Vec::new();
        world.sim.command(Command::Spawn(None));
        for step in 0..1800 {
            match step {
                300 => world.sim.command(Command::Notify(Notification::info("x"))),
                400 => world.sim.command(Command::Play("wave".into())),
                600 => world.sim.command(Command::SetStatus(Status::Thinking)),
                900 => world.sim.command(Command::MoveTo(Vec2::new(300.0, 300.0))),
                _ => {}
            }
            world.cursor = Some(Vec2::new(step as f32, 500.0));
            world.tick();
            path.push(world.sim.position());
        }
        path
    };
    let (mut a, mut b) = (World::new(&config()), World::new(&config()));
    assert_eq!(script(&mut a), script(&mut b));
    assert_eq!(a.events, b.events);
}

fn finished(stunt: Stunt) -> impl Fn(&World) -> bool {
    move |w| w.has(&AgentEvent::StuntFinished(stunt))
}

fn any_finished(world: &World) -> bool {
    world
        .events
        .iter()
        .any(|e| matches!(e, AgentEvent::StuntFinished(_)))
}

fn heist_config() -> AgentConfig {
    AgentConfig {
        cursor_grab: CursorGrabConfig {
            enabled: true,
            ..CursorGrabConfig::default()
        },
        ..config()
    }
}

#[test]
fn roster_is_common_plus_own_species() {
    let rig = character().rig();
    for &stunt in Stunt::ALL {
        let own = Stunt::roster(rig).any(|s| s == stunt);
        assert_eq!(own, stunt.cast().is_none_or(|kind| Some(kind) == rig), "{stunt:?}");
        assert_eq!(Stunt::from_name(stunt.name()), Some(stunt));
    }
}

/// Как на рабочем столе: свободная прогулка, курсор рядом. Карточке и встряске нужны свои условия.
#[test]
fn every_stunt_finishes_while_wandering() {
    let rig = character().rig();
    for stunt in Stunt::roster(rig).filter(|s| !matches!(s, Stunt::TripOverCard | Stunt::ShakeOff))
    {
        let mut world = World::alive(&AgentConfig {
            naughty: true,
            ..heist_config()
        });
        world.cursor = Some(SPAWN_AT + Vec2::new(180.0, -40.0));
        world.tick();
        world.sim.command(Command::Stage(stunt));
        assert!(world.run_until(40.0, finished(stunt)), "{stunt:?}");
    }
}

#[test]
fn foreign_species_stunts_are_refused() {
    let mut world = World::alive(&config());
    let rig = character().rig();
    for stunt in Stunt::ALL.iter().copied().filter(|s| s.cast().is_some_and(|k| Some(k) != rig)) {
        assert!(!world.sim.stage(stunt), "{stunt:?}");
    }
    world.run(1.0);
    let effects = world.sim.effects();
    assert!(effects.leaves.is_empty() && effects.mud.is_empty());
}

#[test]
fn zoomies_circle_twice_and_finish_dizzy() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Zoomies));
    let start = world.sim.position();
    let mut far = 0.0_f32;
    let mut path = 0.0;
    let mut last = start;
    let done = (0..(14.0 / DT) as usize).any(|_| {
        world.tick();
        path += world.sim.position().distance(last);
        last = world.sim.position();
        far = far.max(last.distance(start));
        finished(Stunt::Zoomies)(&world)
    });
    assert!(done);
    // Два круга радиусом 90 рядом с местом, где стоял
    assert!(path > 2.0 * 2.0 * std::f32::consts::PI * 60.0, "{path}");
    assert!(far < 400.0, "{far}");
    assert!(!world.events.iter().any(|e| matches!(e, AgentEvent::Played(_))));
}

#[test]
fn note_brings_a_joke_card_only_onto_an_empty_screen() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Note));
    assert!(world.run_until(3.0, finished(Stunt::Note)));
    assert_eq!(world.sim.items().iter().count(), 1);
    world.events.clear();
    world.sim.command(Command::Stage(Stunt::Note));
    world.run(2.0);
    assert_eq!(world.sim.items().iter().count(), 1, "не вытесняет карточки");
    assert!(!any_finished(&world));
    assert!(world.run_until(8.0, |w| w.has(&closed(false))));
}

#[test]
fn cursor_heist_needs_the_grab_setting() {
    let mut world = World::alive(&config());
    world.cursor = Some(world.sim.position() + Vec2::new(150.0, 0.0));
    world.tick();
    assert!(!world.sim.stage(Stunt::CursorHeist));
    assert!(!world.moves_cursor(10.0));
}

#[test]
fn cursor_heist_catches_the_cursor_carries_it_around_and_lets_go() {
    let mut world = World::alive(&heist_config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world.cursor = Some(world.sim.position() + Vec2::new(160.0, -40.0));
    world.tick();
    assert!(world.sim.stage(Stunt::CursorHeist));
    assert!(world.run_until(8.0, |w| w.sim.grab.holding()));
    let caught = world.sim.position();
    let (mut held, mut far) = (0.0, 0.0_f32);
    while world.tick().is_some() {
        held += DT;
        far = far.max(world.sim.position().distance(caught));
        // Курсор в клюве: не отстаёт от головы дальше шага
        assert!(world.cursor.unwrap().distance(world.sim.head()) < 40.0);
        assert!(held < 7.0);
    }
    assert!((4.5..=5.5).contains(&held), "{held}");
    assert!(far > 150.0, "носился с курсором: {far}");
    assert!(world.run_until(2.0, finished(Stunt::CursorHeist)));
    assert!(!world.moves_cursor(3.0), "брошенный курсор не трогает");
}

#[test]
fn user_takes_the_stolen_cursor_back() {
    let mut world = World::alive(&heist_config());
    world.sim.command(Command::SetBehavior(BehaviorMode::Stay));
    world.cursor = Some(world.sim.position() + Vec2::new(120.0, 0.0));
    world.tick();
    assert!(world.sim.stage(Stunt::CursorHeist));
    assert!(world.run_until(8.0, |w| w.sim.grab.holding()));
    world.run(1.0);
    world.input = true;
    assert!(world.tick().is_none());
    world.input = false;
    assert!(!world.moves_cursor(4.0));
    assert!(!any_finished(&world));
}

#[test]
fn stunt_is_played_while_busy_and_work_resumes() {
    let mut world = World::alive(&config());
    world.sim.command(Command::SetStatus(Status::Busy));
    world.run(1.0);
    world.sim.command(Command::Stage(Stunt::Zoomies));
    assert!(world.run_until(14.0, finished(Stunt::Zoomies)));
    world.run(4.0);
    assert!(world.travelled(3.0) < 1.0, "после сценки снова за работой");
}

#[test]
fn second_stunt_waits_for_the_first_and_drag_cancels_it() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Stage(Stunt::Zoomies));
    world.sim.command(Command::Stage(Stunt::Note));
    world.tick();
    // Вторая команда не принесла записку
    assert!(world.sim.items().is_empty());
    let at = world.sim.position();
    world.cursor = Some(at);
    world.sim.pointer(Pointer::Down(at));
    world.cursor = Some(at + Vec2::new(60.0, 0.0));
    world.run(1.0);
    world.sim.pointer(Pointer::Up(at + Vec2::new(60.0, 0.0)));
    world.run(16.0);
    assert!(!any_finished(&world));
}

#[test]
fn stunt_is_ignored_while_absent() {
    let mut world = World::new(&config());
    for &stunt in Stunt::ALL {
        world.sim.command(Command::Stage(stunt));
    }
    world.sim.command(Command::Surprise);
    world.run(1.0);
    assert!(world.sim.effects().leaves.is_empty());
    assert!(world.sim.is_dormant());
}

#[test]
fn held_state_keeps_the_character_in_place_until_released() {
    let mut world = World::alive(&config());
    world.sim.command(Command::Hold(Some("celebrate".into())));
    world.run(2.0);
    assert!(world.travelled(8.0) < 2.0);
    world.sim.command(Command::Hold(Some("no-such-action".into())));
    assert!(world.travelled(2.0) < 2.0);
    world.sim.command(Command::Hold(None));
    assert!(world.travelled(30.0) > 50.0);
}

#[test]
fn surprise_does_something_and_reports_it() {
    for seed in 0..6 {
        let mut world = World::alive(&AgentConfig {
            seed: Some(seed),
            ..config()
        });
        world.sim.command(Command::Surprise);
        let reported = |w: &World| {
            w.events
                .iter()
                .any(|e| matches!(e, AgentEvent::Played(_) | AgentEvent::StuntFinished(_)))
        };
        assert!(world.run_until(16.0, reported), "зерно {seed}");
    }
}

#[test]
fn character_starts_stunts_on_its_own_only_when_allowed() {
    let started = |stunts: bool| {
        let mut world = World::alive(&AgentConfig { stunts, ..config() });
        world.run_until(900.0, any_finished)
    };
    assert!(started(true));
    assert!(!started(false));
}
