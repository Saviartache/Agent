use super::*;
use crate::math::Rect;
use crate::motion::{Arena, Body, Limits, Mover};

const DT: f32 = 1.0 / 60.0;
/// Столько длится любая разовая анимация в стенде.
const ONE_SHOT_SECS: f32 = 0.5;
const ALL_KINDS: [IntentKind; 6] = [
    IntentKind::Wander,
    IntentKind::Rest,
    IntentKind::ObserveCursor,
    IntentKind::ApproachCursor,
    IntentKind::ChaseCursor,
    IntentKind::Nap,
];

const SPAWN: ActionId = ActionId(0);
const IDLE: ActionId = ActionId(1);
const WALK: ActionId = ActionId(2);
const RUN: ActionId = ActionId(3);
const TURN: ActionId = ActionId(4);
const LOOK: ActionId = ActionId(5);
const REACT: ActionId = ActionId(6);
const INTERACT: ActionId = ActionId(7);
const CELEBRATE: ActionId = ActionId(8);
const THINK: ActionId = ActionId(9);
const BUSY: ActionId = ActionId(10);
const WARNING: ActionId = ActionId(11);
const ERROR: ActionId = ActionId(12);
const SLEEP: ActionId = ActionId(13);
const WAKE_UP: ActionId = ActionId(14);
const DRAGGED: ActionId = ActionId(15);
const DESPAWN: ActionId = ActionId(16);
const ONE_SHOTS: [ActionId; 10] = [
    SPAWN, TURN, LOOK, REACT, INTERACT, CELEBRATE, WARNING, ERROR, WAKE_UP, DESPAWN,
];

fn actions() -> ActionSet {
    ActionSet {
        spawn: SPAWN,
        idle: IDLE,
        walk: WALK,
        run: RUN,
        turn: Some(TURN),
        look: LOOK,
        react: REACT,
        interact: INTERACT,
        celebrate: CELEBRATE,
        think: THINK,
        busy: BUSY,
        warning: WARNING,
        error: ERROR,
        sleep: SLEEP,
        wake_up: Some(WAKE_UP),
        dragged: DRAGGED,
        despawn: Some(DESPAWN),
    }
}

fn open() -> Permissions {
    Permissions {
        cursor_interest: true,
        click_reaction: true,
    }
}

fn at(x: f32, y: f32) -> Option<CursorSense> {
    Some(CursorSense {
        pos: Vec2::new(x, y),
        vel: Vec2::ZERO,
    })
}

/// Мозг и движение вместе, со скриптуемым курсором.
struct Sim {
    brain: Brain,
    mover: Mover,
    body: Body,
    rng: Rng,
    bounds: Bounds,
    cursor: Option<CursorSense>,
    current: ActionId,
    elapsed: f32,
    done: bool,
    arrived: bool,
    never_done: bool,
    frozen: bool,
    time: f32,
    events: Vec<(f32, BrainEvent)>,
    log: Vec<Decision>,
}

impl Sim {
    fn build(actions: ActionSet, permissions: Permissions, seed: u64) -> Self {
        Self {
            brain: Brain::new(BehaviorConfig::default(), actions, permissions, seed),
            mover: Mover::new(),
            body: Body::at(Vec2::new(500.0, 300.0)),
            rng: Rng::new(seed ^ 0xA5),
            bounds: Bounds::new(vec![Rect::from_xywh(0.0, 0.0, 1920.0, 1080.0)]).unwrap(),
            cursor: None,
            current: IDLE,
            elapsed: 0.0,
            done: false,
            arrived: false,
            never_done: false,
            frozen: false,
            time: 0.0,
            events: Vec::new(),
            log: Vec::new(),
        }
    }

    fn new(seed: u64) -> Self {
        Self::build(actions(), open(), seed)
    }

    fn spawned(seed: u64) -> Self {
        Self::alive(Self::new(seed))
    }

    fn alive(mut sim: Self) -> Self {
        sim.push(Directive::Spawn);
        assert!(sim.run_until(5.0, |s| !matches!(
            s.brain.intent(),
            Intent::Absent | Intent::Spawning
        )));
        sim.events.clear();
        sim
    }

    fn only(mut self, keep: &[IntentKind]) -> Self {
        for kind in ALL_KINDS {
            self.brain.set_allowed(kind, keep.contains(&kind));
        }
        self
    }

    fn push(&mut self, directive: Directive) {
        self.brain.push(directive);
    }

    fn tick(&mut self) -> Decision {
        let senses = Senses {
            pos: self.body.pos,
            cursor: self.cursor,
            arrived: self.arrived,
            action_done: self.done && !self.never_done,
            bounds: &self.bounds,
            scale: 1.0,
            staged: false,
        };
        let decision = self.brain.update(DT, &senses);
        self.time += DT;
        if decision.action == self.current {
            self.elapsed += DT;
        } else {
            self.current = decision.action;
            self.elapsed = 0.0;
        }
        self.done = ONE_SHOTS.contains(&decision.action) && self.elapsed >= ONE_SHOT_SECS;
        let limits = match decision.gait {
            Gait::Walk => Limits {
                max_speed: 70.0,
                acceleration: 420.0,
                deceleration: 640.0,
            },
            Gait::Run => Limits {
                max_speed: 230.0,
                acceleration: 420.0,
                deceleration: 640.0,
            },
        };
        if !self.frozen {
            self.arrived = self.mover.step(
                &mut self.body,
                &decision.steer,
                &limits,
                &Arena {
                    bounds: &self.bounds,
                    margin: 56.0,
                },
                &mut self.rng,
                DT,
            );
        }
        let mut fresh = Vec::new();
        self.brain.drain_events(&mut fresh);
        self.events
            .extend(fresh.into_iter().map(|e| (self.time, e)));
        self.log.push(decision);
        decision
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.tick();
        }
    }

    fn run_until(&mut self, secs: f32, done: impl Fn(&Sim) -> bool) -> bool {
        for _ in 0..(secs / DT).round() as usize {
            self.tick();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn reach(&mut self, intent: Intent) {
        assert!(
            self.run_until(10.0, |s| s.brain.intent() == intent),
            "{intent:?}"
        );
    }

    fn count(&self, event: BrainEvent) -> usize {
        self.events.iter().filter(|(_, e)| *e == event).count()
    }

    fn intents(&self) -> Vec<(f32, Intent)> {
        self.events
            .iter()
            .filter_map(|(t, e)| match e {
                BrainEvent::IntentChanged(intent) => Some((*t, *intent)),
                _ => None,
            })
            .collect()
    }

    fn last(&self) -> Decision {
        *self.log.last().unwrap()
    }
}

// --- жизненный цикл ---

#[test]
fn absent_ignores_everything_but_spawn() {
    let mut sim = Sim::new(1);
    for directive in [
        Directive::Click,
        Directive::Mode(BehaviorMode::Stay),
        Directive::Perform(CELEBRATE),
        Directive::Status(Status::Busy),
        Directive::DragStart,
        Directive::MoveTo(Vec2::ZERO),
        Directive::Despawn,
    ] {
        sim.push(directive);
    }
    sim.run(1.0);
    assert_eq!(sim.brain.intent(), Intent::Absent);
    assert_eq!(sim.brain.status(), Status::Normal);
    assert!(sim.events.is_empty());
    let d = sim.last();
    assert_eq!((d.action, d.steer), (IDLE, Steer::Halt));
}

#[test]
fn spawn_and_despawn_cycle() {
    let mut sim = Sim::new(1);
    sim.push(Directive::Spawn);
    sim.tick();
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Spawning, SPAWN)
    );
    assert!(sim.run_until(3.0, |s| s.count(BrainEvent::Spawned) == 1));
    assert!(!matches!(
        sim.brain.intent(),
        Intent::Spawning | Intent::Absent
    ));

    sim.push(Directive::Despawn);
    sim.tick();
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Despawning, DESPAWN)
    );
    assert!(sim.run_until(3.0, |s| s.count(BrainEvent::Despawned) == 1));
    assert_eq!(sim.brain.intent(), Intent::Absent);
    assert_eq!(sim.last().steer, Steer::Halt);

    sim.push(Directive::Spawn);
    assert!(sim.run_until(3.0, |s| s.count(BrainEvent::Spawned) == 2));
}

#[test]
fn despawn_without_action_is_immediate() {
    let mut set = actions();
    set.despawn = None;
    let mut sim = Sim::alive(Sim::build(set, open(), 1));
    sim.push(Directive::Despawn);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Absent);
    assert_eq!(sim.count(BrainEvent::Despawned), 1);
}

#[test]
fn spawn_times_out_without_action_done() {
    let mut sim = Sim::new(1);
    sim.never_done = true;
    sim.push(Directive::Spawn);
    sim.run(3.8);
    assert_eq!(sim.brain.intent(), Intent::Spawning);
    sim.run(0.5);
    assert_eq!(sim.count(BrainEvent::Spawned), 1);
}

#[test]
fn despawn_works_from_any_alive_state() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::DragStart);
    sim.run(0.2);
    sim.push(Directive::Despawn);
    sim.run(0.1);
    assert_eq!(sim.brain.intent(), Intent::Despawning);
}

// --- свободный режим ---

#[test]
fn autonomous_intents_have_sane_dwell() {
    let mut sim = Sim::spawned(3);
    sim.run(600.0);
    let changes = sim.intents();
    assert!(changes.len() >= 30, "{}", changes.len());
    let shortest = changes
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .fold(f32::MAX, f32::min);
    assert!(shortest >= 1.9, "{shortest}");
}

#[test]
fn rest_sometimes_looks() {
    let mut sim = Sim::spawned(4).only(&[IntentKind::Rest]);
    sim.run(60.0);
    assert!(sim.log.iter().any(|d| d.action == LOOK));
}

#[test]
fn observe_halts_and_faces_cursor() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::ObserveCursor]);
    sim.cursor = at(700.0, 300.0);
    sim.reach(Intent::ObserveCursor);
    let d = sim.last();
    assert_eq!(d.steer, Steer::Halt);
    assert_eq!(d.look_at, Some(Vec2::new(700.0, 300.0)));
    assert_eq!(d.facing, Facing::Right);
    sim.cursor = at(300.0, 300.0);
    sim.tick();
    assert_eq!(sim.last().facing, Facing::Left);
}

#[test]
fn cursor_outside_notice_radius_is_not_observed() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::ObserveCursor]);
    sim.cursor = at(1500.0, 300.0);
    sim.run(30.0);
    assert!(
        sim.intents()
            .iter()
            .all(|(_, i)| *i != Intent::ObserveCursor)
    );
}

#[test]
fn approach_makes_contact_then_cools_down() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::ApproachCursor]);
    sim.cursor = at(700.0, 300.0);
    sim.reach(Intent::ApproachCursor);
    assert!(matches!(sim.last().steer, Steer::Arrive { .. }));
    assert_eq!(sim.last().gait, Gait::Walk);
    assert!(
        sim.run_until(10.0, |s| s.count(BrainEvent::Reacted(Reaction::Contact))
            == 1)
    );
    let contact = sim.time;
    assert!(sim.run_until(1.0, |s| s.brain.intent() == Intent::Perform(INTERACT)));
    sim.run(19.0);
    assert!(
        sim.intents()
            .iter()
            .all(|(t, i)| *t < contact || *i != Intent::ApproachCursor)
    );
    sim.run(5.0);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Contact)), 2);
}

#[test]
fn approach_gives_up_on_timeout() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::ApproachCursor]);
    sim.cursor = at(700.0, 300.0);
    sim.frozen = true;
    sim.reach(Intent::ApproachCursor);
    sim.run(13.0);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Contact)), 0);
    assert_ne!(sim.brain.intent(), Intent::ApproachCursor);
}

#[test]
fn chase_runs_touches_once_and_cools_down() {
    let mut grabs = 0;
    for seed in 0..20 {
        let mut sim = Sim::spawned(seed).only(&[IntentKind::ChaseCursor]);
        sim.cursor = at(700.0, 300.0);
        sim.reach(Intent::ChaseCursor);
        assert!(matches!(sim.last().steer, Steer::Pursue { .. }));
        assert_eq!(sim.last().gait, Gait::Run);
        sim.run(7.5);
        assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Contact)), 1);
        assert_ne!(sim.brain.intent(), Intent::ChaseCursor);
        grabs += sim.count(BrainEvent::WantsCursorGrab);
        sim.events.clear();
        sim.run(35.0);
        assert!(sim.intents().iter().all(|(_, i)| *i != Intent::ChaseCursor));
    }
    assert!(grabs > 0 && grabs < 20);
}

#[test]
fn cursor_vanishing_ends_cursor_intents() {
    for kind in [
        IntentKind::ObserveCursor,
        IntentKind::ApproachCursor,
        IntentKind::ChaseCursor,
    ] {
        let mut sim = Sim::spawned(1).only(&[kind]);
        sim.cursor = at(760.0, 300.0);
        assert!(sim.run_until(5.0, |s| matches!(
            s.brain.intent(),
            Intent::ObserveCursor | Intent::ApproachCursor | Intent::ChaseCursor
        )));
        sim.cursor = None;
        sim.tick();
        assert!(!matches!(
            sim.brain.intent(),
            Intent::ObserveCursor | Intent::ApproachCursor | Intent::ChaseCursor
        ));
    }
}

#[test]
fn nap_sleeps_and_wakes_on_click() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    assert_eq!((sim.last().action, sim.last().steer), (SLEEP, Steer::Halt));
    sim.run(2.0);
    assert_eq!(sim.brain.intent(), Intent::Nap);
    sim.push(Directive::Click);
    sim.tick();
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Waking, WAKE_UP)
    );
    sim.run(1.0);
    assert_eq!(sim.brain.intent(), Intent::Rest);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Click)), 0);
}

#[test]
fn nap_wakes_on_status_and_drag() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    sim.push(Directive::Status(Status::Busy));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Waking);
    sim.run(1.0);
    assert_eq!(sim.brain.intent(), Intent::Work);

    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    sim.push(Directive::DragStart);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Dragged);
}

#[test]
fn waking_times_out_without_action_done() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    sim.never_done = true;
    sim.push(Directive::Click);
    sim.run(3.3);
    assert_ne!(sim.brain.intent(), Intent::Waking);
}

#[test]
fn forbidding_a_kind_stops_it_at_once() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    sim.brain.set_allowed(IntentKind::Nap, false);
    sim.tick();
    assert_ne!(sim.brain.intent(), Intent::Nap);

    let mut sim = Sim::spawned(2);
    sim.brain.set_allowed(IntentKind::Wander, false);
    sim.run(300.0);
    assert!(sim.intents().iter().all(|(_, i)| *i != Intent::Wander));
}

// --- испуг и поворот ---

#[test]
fn fast_cursor_startles_with_rate_limit() {
    let dash_near = |sim: &Sim| {
        Some(CursorSense {
            pos: sim.body.pos + Vec2::new(100.0, 0.0),
            vel: Vec2::new(4000.0, 0.0),
        })
    };
    let mut sim = Sim::spawned(1).only(&[IntentKind::Rest]);
    let dash = Some(CursorSense {
        pos: Vec2::new(600.0, 300.0),
        vel: Vec2::new(4000.0, 0.0),
    });
    sim.cursor = dash;
    sim.tick();
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 1);
    assert_eq!(sim.brain.intent(), Intent::FleeCursor);
    assert!(matches!(sim.last().steer, Steer::Flee { .. }));
    assert_eq!(sim.last().gait, Gait::Run);
    sim.cursor = at(600.0, 300.0);
    let start = sim.time;
    assert!(sim.run_until(2.0, |s| s.brain.intent() != Intent::FleeCursor));
    let secs = sim.time - start;
    assert!((0.9..=1.6).contains(&secs), "{secs}");
    sim.cursor = dash_near(&sim);
    sim.run(1.0);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 1);
    sim.run(3.0);
    sim.cursor = dash_near(&sim);
    sim.tick();
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 2);
}

#[test]
fn startle_is_skipped_during_work_drag_and_nap() {
    let dash = Some(CursorSense {
        pos: Vec2::new(600.0, 300.0),
        vel: Vec2::new(4000.0, 0.0),
    });
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Status(Status::Busy));
    sim.run(0.3);
    sim.cursor = dash;
    sim.run(0.3);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 0);

    let mut sim = Sim::spawned(1);
    sim.push(Directive::DragStart);
    sim.run(0.3);
    sim.cursor = dash;
    sim.run(0.3);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 0);

    let mut sim = Sim::spawned(1).only(&[IntentKind::Nap]);
    sim.reach(Intent::Nap);
    sim.cursor = dash;
    sim.run(0.3);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 0);
}

fn longest_turn(sim: &Sim) -> usize {
    sim.log
        .split(|d| d.action != TURN)
        .map(<[Decision]>::len)
        .max()
        .unwrap_or(0)
}

#[test]
fn turn_plays_briefly_while_moving_on() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::MoveTo(Vec2::new(900.0, 300.0)));
    sim.run(2.0);
    assert_eq!(sim.last().facing, Facing::Right);
    sim.log.clear();
    sim.push(Directive::MoveTo(Vec2::new(100.0, 300.0)));
    sim.run(3.0);
    assert!(
        sim.log
            .iter()
            .any(|d| d.action == TURN && d.steer != Steer::Halt)
    );
    assert!(longest_turn(&sim) <= (0.4 / DT) as usize + 2);
    assert_eq!(
        sim.log.iter().find(|d| d.action == WALK).map(|d| d.facing),
        Some(Facing::Left)
    );
}

#[test]
fn no_turn_action_means_no_turn() {
    let mut set = actions();
    set.turn = None;
    let mut sim = Sim::alive(Sim::build(set, open(), 1));
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::MoveTo(Vec2::new(900.0, 300.0)));
    sim.run(2.0);
    sim.push(Directive::MoveTo(Vec2::new(100.0, 300.0)));
    sim.run(3.0);
    assert!(sim.log.iter().all(|d| d.action != TURN));
}

// --- режимы ---

#[test]
fn stay_idles_in_place() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.run(2.0);
    let rest = sim.body.pos;
    sim.run(30.0);
    assert_eq!(sim.brain.intent(), Intent::Stay);
    assert_eq!(sim.body.pos, rest);
    assert_eq!((sim.last().action, sim.last().steer), (IDLE, Steer::Halt));
}

#[test]
fn follow_keeps_a_small_distance() {
    let mut sim = Sim::spawned(1);
    sim.cursor = at(1000.0, 300.0);
    sim.push(Directive::Mode(BehaviorMode::FollowCursor));
    sim.run(25.0);
    assert_eq!(sim.brain.intent(), Intent::Follow);
    let gap = sim.body.pos.distance(Vec2::new(1000.0, 300.0));
    assert!((40.0..=100.0).contains(&gap), "{gap}");
    assert_eq!(sim.last().steer, Steer::Halt);
    assert!(sim.last().look_at.is_some());
}

#[test]
fn directed_chase_pursues_at_a_run() {
    let mut sim = Sim::spawned(1);
    sim.cursor = at(1000.0, 300.0);
    sim.push(Directive::Mode(BehaviorMode::ChaseCursor));
    sim.run(1.0);
    assert_eq!(sim.brain.intent(), Intent::ChaseCursor);
    assert_eq!(sim.last().gait, Gait::Run);
    assert!(matches!(sim.last().steer, Steer::Pursue { .. }));
}

#[test]
fn sleep_mode_naps_until_mode_changes() {
    let mut sim = Sim::spawned(1).only(&[IntentKind::Wander]);
    sim.push(Directive::Mode(BehaviorMode::Sleep));
    sim.run(100.0);
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Nap, SLEEP)
    );
    sim.push(Directive::Mode(BehaviorMode::Wander));
    sim.run(0.2);
    assert_eq!(sim.brain.intent(), Intent::Wander);
}

#[test]
fn cursor_modes_degrade_to_stay() {
    for mode in [BehaviorMode::FollowCursor, BehaviorMode::ChaseCursor] {
        let mut sim = Sim::spawned(1);
        sim.push(Directive::Mode(mode));
        sim.run(0.5);
        assert_eq!(sim.brain.intent(), Intent::Stay);

        let permissions = Permissions {
            cursor_interest: false,
            click_reaction: true,
        };
        let mut sim = Sim::alive(Sim::build(actions(), permissions, 1));
        sim.cursor = at(700.0, 300.0);
        sim.push(Directive::Mode(mode));
        sim.run(0.5);
        assert_eq!(sim.brain.intent(), Intent::Stay);
    }
}

#[test]
fn move_to_walks_there_then_resumes_mode() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    let target = Vec2::new(700.0, 300.0);
    sim.push(Directive::MoveTo(target));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::GoTo(target));
    assert_eq!(sim.last().gait, Gait::Walk);
    assert!(sim.run_until(10.0, |s| s.brain.intent() == Intent::Stay));
    assert!(sim.body.pos.distance(target) <= 6.0);
}

#[test]
fn far_move_to_runs() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::MoveTo(Vec2::new(1800.0, 900.0)));
    sim.tick();
    assert_eq!(sim.last().gait, Gait::Run);
}

#[test]
fn move_to_times_out() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.frozen = true;
    sim.push(Directive::MoveTo(Vec2::new(900.0, 300.0)));
    sim.run(59.0);
    assert!(matches!(sim.brain.intent(), Intent::GoTo(_)));
    sim.run(2.0);
    assert_eq!(sim.brain.intent(), Intent::Stay);
}

#[test]
fn move_to_interrupts_work_and_work_resumes_on_arrival() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Status(Status::Busy));
    sim.run(0.5);
    assert_eq!(sim.brain.intent(), Intent::Work);
    let target = Vec2::new(700.0, 300.0);
    sim.push(Directive::MoveTo(target));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::GoTo(target));
    assert!(sim.run_until(10.0, |s| s.brain.intent() == Intent::Work));
    assert!(sim.body.pos.distance(target) <= 6.0);
}

// --- разовые действия и статусы ---

#[test]
fn perform_plays_once_and_resumes() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::Perform(CELEBRATE));
    sim.push(Directive::Perform(CELEBRATE));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Perform(CELEBRATE));
    assert_eq!(
        (sim.last().action, sim.last().steer),
        (CELEBRATE, Steer::Halt)
    );
    sim.run(2.0);
    assert_eq!(sim.count(BrainEvent::Performed(CELEBRATE)), 1);
    assert_eq!(sim.brain.intent(), Intent::Stay);
}

#[test]
fn perform_times_out_without_action_done() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.never_done = true;
    sim.push(Directive::Perform(CELEBRATE));
    sim.run(5.8);
    assert_eq!(sim.count(BrainEvent::Performed(CELEBRATE)), 0);
    sim.run(0.5);
    assert_eq!(sim.count(BrainEvent::Performed(CELEBRATE)), 1);
}

#[test]
fn work_statuses_hold_until_normal() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::Status(Status::Thinking));
    sim.tick();
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Work, THINK)
    );
    sim.push(Directive::Status(Status::Busy));
    sim.tick();
    assert_eq!(sim.last().action, BUSY);
    sim.push(Directive::Status(Status::Busy));
    sim.run(30.0);
    assert!(sim.log.iter().any(|d| d.action == LOOK));
    assert_eq!(sim.brain.intent(), Intent::Work);
    sim.push(Directive::Status(Status::Normal));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Stay);
}

#[test]
fn work_still_watches_a_nearby_cursor() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Status(Status::Busy));
    sim.cursor = at(300.0, 300.0);
    sim.run(0.5);
    let d = sim.last();
    assert_eq!(d.look_at, Some(Vec2::new(300.0, 300.0)));
    assert_eq!(d.facing, Facing::Left);
}

#[test]
fn momentary_statuses_play_once_and_settle() {
    for (status, action, reaction) in [
        (Status::Success, CELEBRATE, Reaction::Joy),
        (Status::Warning, WARNING, Reaction::Alarm),
        (Status::Error, ERROR, Reaction::Failure),
    ] {
        let mut sim = Sim::spawned(1);
        sim.push(Directive::Mode(BehaviorMode::Stay));
        sim.push(Directive::Status(status));
        sim.tick();
        assert_eq!(sim.last().action, action);
        assert_eq!(sim.brain.status(), status);
        sim.run(2.0);
        assert_eq!(sim.count(BrainEvent::Reacted(reaction)), 1);
        assert_eq!(sim.count(BrainEvent::StatusSettled), 1);
        assert_eq!(sim.brain.status(), Status::Normal);
        assert_eq!(sim.brain.intent(), Intent::Stay);
    }
}

#[test]
fn success_after_busy_ends_work_and_resumes_previous_activity() {
    let mut sim = Sim::spawned(2).only(&[IntentKind::Wander]);
    sim.reach(Intent::Wander);
    sim.push(Directive::Status(Status::Busy));
    sim.run(2.0);
    assert_eq!(sim.brain.intent(), Intent::Work);
    sim.push(Directive::Status(Status::Success));
    sim.run(2.0);
    assert_eq!(sim.brain.status(), Status::Normal);
    assert_eq!(sim.brain.intent(), Intent::Wander);
    sim.run(20.0);
    assert_ne!(sim.brain.intent(), Intent::Work);
}

#[test]
fn momentary_reaction_times_out_without_action_done() {
    let mut sim = Sim::spawned(1);
    sim.never_done = true;
    sim.push(Directive::Status(Status::Error));
    sim.run(6.3);
    assert_eq!(sim.count(BrainEvent::StatusSettled), 1);
}

// --- клик и перетаскивание ---

#[test]
fn click_reacts_with_cooldown() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::Click);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Perform(REACT));
    sim.run(1.0);
    sim.push(Directive::Click);
    sim.run(0.5);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Click)), 1);
    sim.run(2.0);
    sim.push(Directive::Click);
    sim.run(0.5);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Click)), 2);
    assert_eq!(sim.count(BrainEvent::Performed(REACT)), 0);
}

#[test]
fn click_reaction_can_be_forbidden() {
    let permissions = Permissions {
        cursor_interest: true,
        click_reaction: false,
    };
    let mut sim = Sim::alive(Sim::build(actions(), permissions, 1));
    sim.push(Directive::Click);
    sim.run(1.0);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Click)), 0);
}

#[test]
fn click_still_reacts_during_work() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Status(Status::Busy));
    sim.run(0.5);
    sim.push(Directive::Click);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Perform(REACT));
    sim.run(1.5);
    assert_eq!(sim.brain.intent(), Intent::Work);
}

#[test]
fn drag_holds_then_rests_then_resumes() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Mode(BehaviorMode::Stay));
    sim.push(Directive::DragStart);
    sim.push(Directive::Perform(CELEBRATE));
    sim.push(Directive::Status(Status::Success));
    sim.run(2.0);
    assert_eq!(
        (sim.brain.intent(), sim.last().action),
        (Intent::Dragged, DRAGGED)
    );
    assert_eq!(sim.last().steer, Steer::Halt);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Joy)), 0);
    sim.push(Directive::DragEnd);
    sim.run(1.5);
    // Статус, пришедший во время переноса, отыгрывается после него
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Joy)), 1);
    assert_eq!(sim.count(BrainEvent::Performed(CELEBRATE)), 0);
    assert!(sim.run_until(5.0, |s| s.brain.intent() == Intent::Rest));
    assert!(sim.run_until(5.0, |s| s.brain.intent() == Intent::Stay));
}

#[test]
fn higher_layers_preempt_lower_ones() {
    let mut sim = Sim::spawned(1);
    sim.push(Directive::Status(Status::Busy));
    sim.push(Directive::Perform(CELEBRATE));
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Perform(CELEBRATE));
    sim.run(1.5);
    assert_eq!(sim.brain.intent(), Intent::Work);

    sim.push(Directive::Status(Status::Warning));
    sim.push(Directive::Perform(CELEBRATE));
    sim.tick();
    assert_eq!(sim.last().action, WARNING);
    sim.push(Directive::DragStart);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Dragged);
    sim.push(Directive::Despawn);
    sim.tick();
    assert_eq!(sim.brain.intent(), Intent::Despawning);
}

// --- очередь, детерминизм, права ---

#[test]
fn queue_overflow_keeps_despawn() {
    let mut brain = Brain::new(BehaviorConfig::default(), actions(), open(), 1);
    brain.push(Directive::Despawn);
    for _ in 0..100 {
        brain.push(Directive::Click);
    }
    assert_eq!(brain.queue.len(), QUEUE_CAP);
    assert!(brain.queue.contains(&Directive::Despawn));

    let mut full = Brain::new(BehaviorConfig::default(), actions(), open(), 1);
    for _ in 0..40 {
        full.push(Directive::Despawn);
    }
    full.push(Directive::Click);
    assert_eq!(full.queue.len(), QUEUE_CAP);
    assert!(full.queue.iter().all(|d| *d == Directive::Despawn));

    let mut sim = Sim::spawned(1);
    sim.push(Directive::Despawn);
    for _ in 0..100 {
        sim.push(Directive::Click);
    }
    sim.run(3.0);
    assert_eq!(sim.count(BrainEvent::Despawned), 1);
}

#[test]
fn events_stay_bounded() {
    let mut brain = Brain::new(BehaviorConfig::default(), actions(), open(), 1);
    for _ in 0..1000 {
        brain.emit(BrainEvent::StatusSettled);
    }
    let mut out = Vec::new();
    brain.drain_events(&mut out);
    assert_eq!(out.len(), EVENT_CAP);
}

fn scripted(seed: u64) -> (Vec<Decision>, Vec<(f32, BrainEvent)>) {
    let mut sim = Sim::spawned(seed);
    for frame in 0..60 * 120 {
        let t = frame as f32 * DT;
        let pos = Vec2::new(700.0 + 200.0 * t.sin(), 300.0 + 120.0 * (t * 0.7).cos());
        let vel = Vec2::new(200.0 * t.cos(), -84.0 * (t * 0.7).sin());
        sim.cursor = (t % 50.0 < 40.0).then_some(CursorSense { pos, vel });
        if frame % 420 == 0 {
            sim.push(Directive::Click);
        }
        sim.tick();
    }
    (sim.log, sim.events)
}

#[test]
fn same_seed_same_stream_different_seed_differs() {
    assert_eq!(scripted(7), scripted(7));
    assert_ne!(scripted(7), scripted(8));
}

#[test]
fn cursor_intents_never_happen_without_interest() {
    let permissions = Permissions {
        cursor_interest: false,
        click_reaction: true,
    };
    let mut sim = Sim::alive(Sim::build(actions(), permissions, 5));
    sim.cursor = Some(CursorSense {
        pos: sim.body.pos,
        vel: Vec2::new(5000.0, 0.0),
    });
    sim.run(600.0);
    assert!(sim.intents().iter().all(|(_, i)| !matches!(
        i,
        Intent::ObserveCursor | Intent::ApproachCursor | Intent::ChaseCursor | Intent::FleeCursor
    )));
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Startle)), 0);
    assert_eq!(sim.count(BrainEvent::Reacted(Reaction::Contact)), 0);
}

#[test]
fn bad_inputs_do_not_break_the_brain() {
    let mut sim = Sim::spawned(1);
    let bounds = sim.bounds.clone();
    for (dt, scale) in [
        (0.0, 1.0),
        (-1.0, 1.0),
        (f32::NAN, 1.0),
        (1e6, 0.0),
        (0.1, f32::NAN),
    ] {
        let senses = Senses {
            pos: Vec2::ZERO,
            cursor: at(10.0, 10.0),
            arrived: false,
            action_done: false,
            bounds: &bounds,
            scale,
            staged: false,
        };
        sim.brain.update(dt, &senses);
    }
    assert!(sim.brain.now.is_finite());
}
