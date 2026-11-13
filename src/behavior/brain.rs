//! Мозг: слои по приоритету (сверху вниз) — перетаскивание, разовая реакция на статус,
//! разовое действие, переход, работа, испуг, `MoveTo`, режим, свободное намерение.
//! Верхний слой замораживает нижние, а не сбрасывает: после него дело продолжается.

use std::collections::VecDeque;

use super::intent::{Facing, Gait, Intent, Reaction};
use super::scheduler::{IntentKind, Scheduler};
use crate::action::{ActionId, ActionSet};
use crate::command::{BehaviorMode, Status};
use crate::config::BehaviorConfig;
use crate::math::{Bounds, Vec2};
use crate::motion::Steer;
use crate::rng::Rng;

const QUEUE_CAP: usize = 32;
const EVENT_CAP: usize = 64;
/// Длиннее за кадр не считаем (после сна машины и т. п.), с.
const MAX_DT: f32 = 0.5;
/// Предохранители: ничто не висит, если `action_done`/`arrived` не пришли, с.
const SPAWN_SECS: f32 = 4.0;
const PERFORM_SECS: f32 = 6.0;
const WAKE_SECS: f32 = 3.0;
const LOOK_SECS: f32 = 3.0;
const TURN_SECS: f32 = 0.4;
const GOTO_SECS: f32 = 60.0;
const DRAG_REST_SECS: f32 = 1.2;
const FLEE_SECS: [f32; 2] = [1.0, 1.5];
/// Пауза между случайными взглядами во время отдыха или работы, с.
const LOOK_GAP_SECS: [f32; 2] = [2.5, 5.0];
const GRAB_CHANCE: f32 = 0.35;
/// Расстояния манифеста, логические px (множатся на `Senses::scale`).
const SLOW_RADIUS: f32 = 90.0;
const GOTO_REACH: f32 = 6.0;
const FAR: f32 = 400.0;
const FOLLOW_KEEP: f32 = 70.0;
const FOLLOW_SLACK: f32 = 24.0;
const LOOK_DEAD_ZONE: f32 = 8.0;
/// Доля горизонтали в направлении хода, после которой взгляд меняет сторону.
const FACE_SLACK: f32 = 0.35;
/// Сглаживание направления хода, 1/с.
const TRAVEL_RATE: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Directive {
    Spawn,
    Despawn,
    Mode(BehaviorMode),
    Perform(ActionId),
    MoveTo(Vec2),
    /// Как `MoveTo`, но бегом с первого шага.
    RunTo(Vec2),
    Status(Status),
    /// Держать действие, пока не придёт `Hold(None)`: своё состояние приложения.
    Hold(Option<ActionId>),
    Click,
    DragStart,
    DragEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorSense {
    pub pos: Vec2,
    pub vel: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct Senses<'a> {
    pub pos: Vec2,
    /// `None` — курсор недоступен (экран блокировки): все намерения вокруг него кончаются.
    pub cursor: Option<CursorSense>,
    /// Итог прошлого `Mover::step`.
    pub arrived: bool,
    /// Разовая анимация действия, выданного прошлым `update`, доиграла.
    pub action_done: bool,
    pub bounds: &'a Bounds,
    /// Множитель логических расстояний конфигурации.
    pub scale: f32,
    /// Идёт сценка: телом правит она, вне её походов персонаж стоит.
    pub staged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub action: ActionId,
    pub steer: Steer,
    pub gait: Gait,
    pub facing: Facing,
    pub look_at: Option<Vec2>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrainEvent {
    Spawned,
    Despawned,
    IntentChanged(Intent),
    Reacted(Reaction),
    Performed(ActionId),
    StatusSettled,
    /// Мозг лишь хочет схватить курсор; решает другой слой.
    WantsCursorGrab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions {
    pub cursor_interest: bool,
    pub click_reaction: bool,
}

/// Разовая анимация с предохранителем по времени.
#[derive(Clone, Copy, Debug)]
struct Shot {
    action: ActionId,
    left: f32,
    issued: bool,
}

impl Shot {
    fn new(action: ActionId, secs: f32) -> Self {
        Self {
            action,
            left: secs,
            issued: false,
        }
    }

    /// Звать раньше выдачи действия. Первый вызов `done` не читает: он про прошлое действие.
    fn poll(&mut self, done: bool, dt: f32) -> bool {
        if !self.issued {
            self.issued = true;
            return false;
        }
        self.left -= dt;
        done || self.left <= 0.0
    }
}

#[derive(Clone, Copy, Debug)]
enum Life {
    Absent,
    Spawning(Shot),
    Alive,
    Despawning(Option<Shot>),
}

#[derive(Clone, Copy, Debug)]
struct Perform {
    shot: Shot,
    /// Сообщать `Performed` — только для директивы приложения.
    announce: bool,
}

#[derive(Clone, Copy, Debug)]
enum Interlude {
    Waking(Shot),
    Rest(f32),
}

#[derive(Clone, Copy, Debug)]
struct Goto {
    target: Vec2,
    left: f32,
    issued: bool,
    run: bool,
}

#[derive(Clone, Copy, Debug)]
struct Auto {
    kind: IntentKind,
    left: f32,
}

/// Случайные взгляды `look` на фоне покоя.
#[derive(Clone, Copy, Debug, Default)]
struct Idler {
    owner: Option<Intent>,
    gap: f32,
    shot: Option<Shot>,
}

/// Что делать в этом кадре, пока без поворота и стороны.
#[derive(Clone, Copy, Debug)]
struct Plan {
    action: ActionId,
    steer: Steer,
    gait: Gait,
    look_at: Option<Vec2>,
}

impl Plan {
    fn stand(action: ActionId) -> Self {
        Self {
            action,
            steer: Steer::Halt,
            gait: Gait::Walk,
            look_at: None,
        }
    }

    fn looking(mut self, at: Option<Vec2>) -> Self {
        self.look_at = at;
        self
    }
}

type Tick = Option<(Intent, Plan)>;

pub struct Brain {
    config: BehaviorConfig,
    actions: ActionSet,
    permissions: Permissions,
    rng: Rng,
    scheduler: Scheduler,
    queue: VecDeque<Directive>,
    events: Vec<BrainEvent>,
    now: f32,
    life: Life,
    mode: BehaviorMode,
    status: Status,
    intent: Intent,
    drag: bool,
    reaction: Option<(Reaction, Shot)>,
    perform: Option<Perform>,
    interlude: Option<Interlude>,
    goto: Option<Goto>,
    hold: Option<ActionId>,
    flee: Option<f32>,
    auto: Option<Auto>,
    contacted: bool,
    idler: Idler,
    turn: Option<Shot>,
    facing: Facing,
    travel: Vec2,
    last_pos: Option<Vec2>,
    click_ready: f32,
    startle_ready: f32,
}

impl Brain {
    pub fn new(
        config: BehaviorConfig,
        actions: ActionSet,
        permissions: Permissions,
        seed: u64,
    ) -> Self {
        Self {
            scheduler: Scheduler::new(&config),
            config,
            actions,
            permissions,
            rng: Rng::new(seed),
            queue: VecDeque::with_capacity(QUEUE_CAP),
            events: Vec::with_capacity(EVENT_CAP),
            now: 0.0,
            life: Life::Absent,
            mode: BehaviorMode::Wander,
            status: Status::Normal,
            intent: Intent::Absent,
            drag: false,
            reaction: None,
            perform: None,
            interlude: None,
            goto: None,
            hold: None,
            flee: None,
            auto: None,
            contacted: false,
            idler: Idler::default(),
            turn: None,
            facing: Facing::Right,
            travel: Vec2::new(1.0, 0.0),
            last_pos: None,
            click_ready: 0.0,
            startle_ready: 0.0,
        }
    }

    /// Переполнение вытесняет самую старую директиву, но не `Despawn`.
    pub fn push(&mut self, directive: Directive) {
        if self.queue.len() == QUEUE_CAP {
            match self.queue.iter().position(|d| *d != Directive::Despawn) {
                Some(index) => {
                    self.queue.remove(index);
                }
                None => return,
            }
        }
        self.queue.push_back(directive);
    }

    pub fn update(&mut self, dt: f32, senses: &Senses<'_>) -> Decision {
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(MAX_DT)
        } else {
            0.0
        };
        let s = Senses {
            cursor: senses.cursor.filter(|_| self.permissions.cursor_interest),
            scale: if senses.scale.is_finite() && senses.scale > 0.0 {
                senses.scale
            } else {
                1.0
            },
            ..*senses
        };
        self.now += dt;
        while let Some(directive) = self.queue.pop_front() {
            self.apply(directive);
        }
        let (intent, plan) = self.lifecycle(dt, &s).unwrap_or_else(|| self.live(dt, &s));
        self.set_intent(intent);
        self.compose(plan, dt, &s)
    }

    pub fn drain_events(&mut self, out: &mut Vec<BrainEvent>) {
        out.append(&mut self.events);
    }

    pub fn intent(&self) -> Intent {
        self.intent
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn set_allowed(&mut self, kind: IntentKind, allowed: bool) {
        self.scheduler.set_enabled(kind, allowed);
        if !allowed && self.auto.is_some_and(|auto| auto.kind == kind) {
            self.end_auto();
        }
    }

    fn emit(&mut self, event: BrainEvent) {
        if self.events.len() == EVENT_CAP {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    fn set_intent(&mut self, intent: Intent) {
        if intent != self.intent {
            self.intent = intent;
            self.emit(BrainEvent::IntentChanged(intent));
        }
    }

    // --- директивы ---

    fn apply(&mut self, directive: Directive) {
        match (self.life, directive) {
            (Life::Absent, Directive::Spawn) => {
                self.life = Life::Spawning(Shot::new(self.actions.spawn, SPAWN_SECS));
            }
            (Life::Absent | Life::Despawning(_), _) => {}
            (_, Directive::Despawn) => {
                self.life = Life::Despawning(
                    self.actions
                        .despawn
                        .map(|action| Shot::new(action, SPAWN_SECS)),
                );
            }
            (_, Directive::Mode(mode)) => self.set_mode(mode),
            (_, Directive::Status(status)) => self.set_status(status),
            (_, Directive::Hold(action)) => self.set_hold(action),
            (Life::Alive, directive) => self.apply_alive(directive),
            (Life::Spawning(_), _) => {}
        }
    }

    fn apply_alive(&mut self, directive: Directive) {
        match directive {
            Directive::Spawn
            | Directive::Despawn
            | Directive::Mode(_)
            | Directive::Status(_)
            | Directive::Hold(_) => {}
            Directive::Perform(action) => self.start_perform(action),
            Directive::MoveTo(target) => self.start_goto(target, false),
            Directive::RunTo(target) => self.start_goto(target, true),
            Directive::Click => self.click(),
            Directive::DragStart => self.start_drag(),
            Directive::DragEnd => self.end_drag(),
        }
    }

    fn start_goto(&mut self, target: Vec2, run: bool) {
        self.goto = Some(Goto {
            target,
            left: GOTO_SECS,
            issued: false,
            run,
        });
    }

    fn set_hold(&mut self, action: Option<ActionId>) {
        self.hold = action;
        if action.is_some() {
            self.wake();
        }
    }

    fn set_mode(&mut self, mode: BehaviorMode) {
        if mode != self.mode {
            self.mode = mode;
            self.end_auto();
        }
    }

    fn set_status(&mut self, status: Status) {
        if status == self.status {
            return;
        }
        let momentary = match status {
            Status::Success => Some((Reaction::Joy, self.actions.celebrate)),
            Status::Warning => Some((Reaction::Alarm, self.actions.warning)),
            Status::Error => Some((Reaction::Failure, self.actions.error)),
            _ => None,
        };
        if let Some((reaction, action)) = momentary {
            self.reaction = Some((reaction, Shot::new(action, PERFORM_SECS)));
        }
        self.status = status;
        if status != Status::Normal {
            self.wake();
        }
    }

    fn start_perform(&mut self, action: ActionId) {
        let repeated = self
            .perform
            .is_some_and(|p| p.announce && p.shot.action == action);
        if !self.drag && !repeated {
            self.perform = Some(Perform {
                shot: Shot::new(action, PERFORM_SECS),
                announce: true,
            });
        }
    }

    fn click(&mut self) {
        if self.drag {
            return;
        }
        if self.intent == Intent::Nap {
            self.wake();
            return;
        }
        let busy = self.perform.is_some() || self.reaction.is_some();
        if !self.permissions.click_reaction || busy || self.now < self.click_ready {
            return;
        }
        self.click_ready = self.now + self.config.react_cooldown_secs;
        self.emit(BrainEvent::Reacted(Reaction::Click));
        self.perform = Some(Perform {
            shot: Shot::new(self.actions.react, PERFORM_SECS),
            announce: false,
        });
    }

    fn start_drag(&mut self) {
        if self.drag {
            return;
        }
        if self.intent == Intent::Nap {
            self.end_auto();
        }
        self.drag = true;
        self.perform = None;
        self.flee = None;
        self.interlude = None;
    }

    fn end_drag(&mut self) {
        if self.drag {
            self.drag = false;
            self.interlude = Some(Interlude::Rest(DRAG_REST_SECS));
        }
    }

    /// Просыпается, если спал; без `wake_up` — просто встаёт.
    fn wake(&mut self) {
        if self.intent != Intent::Nap {
            return;
        }
        if self.auto.is_some_and(|auto| auto.kind == IntentKind::Nap) {
            self.end_auto();
        }
        self.interlude = self
            .actions
            .wake_up
            .map(|action| Interlude::Waking(Shot::new(action, WAKE_SECS)));
    }

    // --- жизненный цикл ---

    fn lifecycle(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let idle = Plan::stand(self.actions.idle);
        match self.life {
            Life::Absent => Some((Intent::Absent, idle)),
            Life::Alive => None,
            Life::Spawning(mut shot) => {
                if shot.poll(s.action_done, dt) {
                    self.life = Life::Alive;
                    self.emit(BrainEvent::Spawned);
                    return None;
                }
                self.life = Life::Spawning(shot);
                Some((Intent::Spawning, Plan::stand(shot.action)))
            }
            Life::Despawning(None) => Some(self.finish_despawn()),
            Life::Despawning(Some(mut shot)) => {
                if shot.poll(s.action_done, dt) {
                    return Some(self.finish_despawn());
                }
                self.life = Life::Despawning(Some(shot));
                Some((Intent::Despawning, Plan::stand(shot.action)))
            }
        }
    }

    fn finish_despawn(&mut self) -> (Intent, Plan) {
        self.life = Life::Absent;
        self.mode = BehaviorMode::Wander;
        self.status = Status::Normal;
        self.drag = false;
        self.reaction = None;
        self.perform = None;
        self.interlude = None;
        self.goto = None;
        self.hold = None;
        self.flee = None;
        self.auto = None;
        self.idler = Idler::default();
        self.turn = None;
        self.facing = Facing::Right;
        self.travel = Vec2::new(1.0, 0.0);
        self.last_pos = None;
        self.emit(BrainEvent::Despawned);
        (Intent::Absent, Plan::stand(self.actions.idle))
    }

    // --- слои живого персонажа ---

    fn live(&mut self, dt: f32, s: &Senses<'_>) -> (Intent, Plan) {
        self.check_startle(s);
        if self.drag {
            return (Intent::Dragged, Plan::stand(self.actions.dragged));
        }
        self.tick_reaction(dt, s)
            .or_else(|| self.tick_perform(dt, s))
            .or_else(|| self.tick_interlude(dt, s))
            // Явный поход (команда, сценка) отвлекает и от работы; дойдя, персонаж к ней вернётся
            .or_else(|| self.tick_goto(dt, s))
            .or_else(|| {
                s.staged
                    .then(|| (Intent::Stay, Plan::stand(self.actions.idle)))
            })
            .or_else(|| self.tick_work(dt, s))
            .or_else(|| self.tick_flee(dt, s))
            .unwrap_or_else(|| self.tick_mode(dt, s))
    }

    fn tick_reaction(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let (reaction, mut shot) = self.reaction?;
        if !shot.issued {
            self.emit(BrainEvent::Reacted(reaction));
        }
        if shot.poll(s.action_done, dt) {
            self.reaction = None;
            if self.status.is_momentary() {
                self.status = Status::Normal;
            }
            self.emit(BrainEvent::StatusSettled);
            return None;
        }
        self.reaction = Some((reaction, shot));
        Some((Intent::Perform(shot.action), Plan::stand(shot.action)))
    }

    fn tick_perform(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let mut perform = self.perform?;
        if perform.shot.poll(s.action_done, dt) {
            self.perform = None;
            if perform.announce {
                self.emit(BrainEvent::Performed(perform.shot.action));
            }
            return None;
        }
        let action = perform.shot.action;
        self.perform = Some(perform);
        Some((Intent::Perform(action), Plan::stand(action)))
    }

    fn tick_interlude(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        match self.interlude? {
            Interlude::Waking(mut shot) => {
                if shot.poll(s.action_done, dt) {
                    self.interlude = None;
                    return None;
                }
                self.interlude = Some(Interlude::Waking(shot));
                Some((Intent::Waking, Plan::stand(shot.action)))
            }
            Interlude::Rest(left) => {
                if left <= dt {
                    self.interlude = None;
                    return None;
                }
                self.interlude = Some(Interlude::Rest(left - dt));
                Some((Intent::Rest, Plan::stand(self.actions.idle)))
            }
        }
    }

    fn tick_work(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let base = match (self.hold, self.status) {
            (Some(action), _) => action,
            (None, Status::Thinking) => self.actions.think,
            (None, Status::Busy) => self.actions.busy,
            _ => return None,
        };
        let action = self.idle_loop(Intent::Work, base, dt, s.action_done);
        Some((
            Intent::Work,
            Plan::stand(action).looking(self.near_cursor(s)),
        ))
    }

    fn tick_flee(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let left = self.flee? - dt;
        let Some(cursor) = s.cursor.filter(|_| left > 0.0) else {
            self.flee = None;
            return None;
        };
        self.flee = Some(left);
        let steer = Steer::Flee { from: cursor.pos };
        Some((Intent::FleeCursor, self.go(steer, Gait::Run)))
    }

    fn tick_goto(&mut self, dt: f32, s: &Senses<'_>) -> Tick {
        let mut goto = self.goto?;
        goto.left -= dt;
        let target = s.bounds.clamp(goto.target, 0.0);
        let distance = s.pos.distance(target);
        if (goto.issued && s.arrived) || distance <= GOTO_REACH * s.scale || goto.left <= 0.0 {
            self.goto = None;
            return None;
        }
        goto.issued = true;
        self.goto = Some(goto);
        let gait = if goto.run || distance > FAR * s.scale {
            Gait::Run
        } else {
            Gait::Walk
        };
        // Поход ведёт в точку, а не в зону касания
        let steer = Steer::Arrive {
            target,
            slow_radius: SLOW_RADIUS * s.scale,
            tolerance: GOTO_REACH * s.scale,
        };
        Some((Intent::GoTo(goto.target), self.go(steer, gait)))
    }

    fn tick_mode(&mut self, dt: f32, s: &Senses<'_>) -> (Intent, Plan) {
        match (self.mode, s.cursor) {
            (BehaviorMode::Wander, _) => self.tick_auto(dt, s),
            (BehaviorMode::Sleep, _) => (Intent::Nap, Plan::stand(self.actions.sleep)),
            (BehaviorMode::FollowCursor, Some(cursor)) => self.follow(cursor, s),
            (BehaviorMode::ChaseCursor, Some(cursor)) => {
                (Intent::ChaseCursor, self.pursue(cursor, s))
            }
            _ => (Intent::Stay, Plan::stand(self.actions.idle)),
        }
    }

    fn follow(&self, cursor: CursorSense, s: &Senses<'_>) -> (Intent, Plan) {
        let away = s.pos - cursor.pos;
        let keep = FOLLOW_KEEP * s.scale;
        if away.length() <= keep + FOLLOW_SLACK * s.scale {
            let idle = Plan::stand(self.actions.idle).looking(Some(cursor.pos));
            return (Intent::Follow, idle);
        }
        let target = cursor.pos + away.normalized() * keep;
        (Intent::Follow, self.arrive(target, Gait::Walk, s))
    }

    // --- свободный режим ---

    fn tick_auto(&mut self, dt: f32, s: &Senses<'_>) -> (Intent, Plan) {
        let mut current = self.auto.map(|mut auto| {
            auto.left -= dt;
            auto
        });
        if let Some(auto) = current
            && !self.auto_valid(auto, s)
        {
            self.end_auto();
            current = None;
        }
        let auto = current.unwrap_or_else(|| self.begin_auto(s));
        self.auto = Some(auto);
        self.run_auto(auto.kind, dt, s)
    }

    fn auto_valid(&self, auto: Auto, s: &Senses<'_>) -> bool {
        auto.left > 0.0
            && match auto.kind {
                IntentKind::ObserveCursor => self.near_cursor(s).is_some(),
                IntentKind::ApproachCursor | IntentKind::ChaseCursor => s.cursor.is_some(),
                _ => true,
            }
    }

    fn begin_auto(&mut self, s: &Senses<'_>) -> Auto {
        let near = self.near_cursor(s).is_some();
        let (kind, left) = self.scheduler.pick(
            self.now,
            |kind| {
                near || !matches!(
                    kind,
                    IntentKind::ObserveCursor
                        | IntentKind::ApproachCursor
                        | IntentKind::ChaseCursor
                )
            },
            &mut self.rng,
        );
        self.contacted = false;
        Auto { kind, left }
    }

    fn end_auto(&mut self) {
        if let Some(auto) = self.auto.take() {
            self.scheduler.finish(auto.kind, self.now);
        }
    }

    fn run_auto(&mut self, kind: IntentKind, dt: f32, s: &Senses<'_>) -> (Intent, Plan) {
        let cursor = s.cursor;
        match (kind, cursor) {
            (IntentKind::Wander, _) => (Intent::Wander, self.go(Steer::Wander, Gait::Walk)),
            (IntentKind::Rest, _) => {
                let action = self.idle_loop(Intent::Rest, self.actions.idle, dt, s.action_done);
                (Intent::Rest, Plan::stand(action))
            }
            (IntentKind::Nap, _) => (Intent::Nap, Plan::stand(self.actions.sleep)),
            (IntentKind::ObserveCursor, Some(c)) => (
                Intent::ObserveCursor,
                Plan::stand(self.actions.idle).looking(Some(c.pos)),
            ),
            (IntentKind::ApproachCursor, Some(c)) => {
                if self.in_contact(c, s) {
                    self.contact();
                    self.end_auto();
                }
                (Intent::ApproachCursor, self.arrive(c.pos, Gait::Walk, s))
            }
            (IntentKind::ChaseCursor, Some(c)) => {
                if !self.contacted && self.in_contact(c, s) {
                    self.contacted = true;
                    self.contact();
                    if self.rng.chance(GRAB_CHANCE) {
                        self.emit(BrainEvent::WantsCursorGrab);
                    }
                }
                (Intent::ChaseCursor, self.pursue(c, s))
            }
            // Курсор исчез между проверкой и ходом: стоим до следующего кадра
            (_, None) => (Intent::Rest, Plan::stand(self.actions.idle)),
        }
    }

    fn contact(&mut self) {
        self.emit(BrainEvent::Reacted(Reaction::Contact));
        self.perform = Some(Perform {
            shot: Shot::new(self.actions.interact, PERFORM_SECS),
            announce: false,
        });
    }

    fn check_startle(&mut self, s: &Senses<'_>) {
        let Some(cursor) = s.cursor else { return };
        let calm = matches!(self.life, Life::Alive)
            && !self.drag
            && self.reaction.is_none()
            && self.perform.is_none()
            && self.interlude.is_none()
            && self.flee.is_none()
            && self.hold.is_none()
            && !matches!(self.status, Status::Thinking | Status::Busy)
            && self.intent != Intent::Nap;
        if !calm || self.now < self.startle_ready {
            return;
        }
        let near = cursor.pos.distance(s.pos) <= self.config.notice_radius * s.scale;
        if near && cursor.vel.length() > self.config.startle_speed * s.scale {
            self.startle_ready = self.now + self.config.react_cooldown_secs;
            self.flee = Some(self.rng.range(FLEE_SECS[0], FLEE_SECS[1]));
            self.emit(BrainEvent::Reacted(Reaction::Startle));
        }
    }

    // --- общее ---

    fn go(&self, steer: Steer, gait: Gait) -> Plan {
        let action = match gait {
            Gait::Walk => self.actions.walk,
            Gait::Run => self.actions.run,
        };
        Plan {
            action,
            steer,
            gait,
            look_at: None,
        }
    }

    fn arrive(&self, target: Vec2, gait: Gait, s: &Senses<'_>) -> Plan {
        let steer = Steer::Arrive {
            target,
            slow_radius: SLOW_RADIUS * s.scale,
            tolerance: self.config.contact_radius * 0.5 * s.scale,
        };
        self.go(steer, gait)
    }

    fn pursue(&self, cursor: CursorSense, s: &Senses<'_>) -> Plan {
        let steer = Steer::Pursue {
            target: cursor.pos,
            target_vel: cursor.vel,
            tolerance: self.config.contact_radius * 0.5 * s.scale,
        };
        self.go(steer, Gait::Run)
    }

    fn in_contact(&self, cursor: CursorSense, s: &Senses<'_>) -> bool {
        cursor.pos.distance(s.pos) <= self.config.contact_radius * s.scale
    }

    fn near_cursor(&self, s: &Senses<'_>) -> Option<Vec2> {
        s.cursor
            .filter(|c| c.pos.distance(s.pos) <= self.config.notice_radius * s.scale)
            .map(|c| c.pos)
    }

    /// Базовое действие покоя, иногда прерываемое разовым `look`.
    fn idle_loop(&mut self, owner: Intent, base: ActionId, dt: f32, done: bool) -> ActionId {
        if self.idler.owner != Some(owner) {
            self.idler = Idler {
                owner: Some(owner),
                gap: self.rng.range(LOOK_GAP_SECS[0], LOOK_GAP_SECS[1]),
                shot: None,
            };
        }
        if let Some(mut shot) = self.idler.shot {
            if !shot.poll(done, dt) {
                self.idler.shot = Some(shot);
                return shot.action;
            }
            self.idler.shot = None;
            self.idler.gap = self.rng.range(LOOK_GAP_SECS[0], LOOK_GAP_SECS[1]);
        }
        self.idler.gap -= dt;
        if self.idler.gap <= 0.0 {
            self.idler.shot = Some(Shot::new(self.actions.look, LOOK_SECS));
        }
        base
    }

    // --- сторона и поворот ---

    fn compose(&mut self, plan: Plan, dt: f32, s: &Senses<'_>) -> Decision {
        let before = self.facing;
        self.update_facing(&plan, dt, s);
        let moving = !matches!(plan.steer, Steer::Halt);
        if moving
            && self.facing != before
            && let Some(action) = self.actions.turn
        {
            self.turn = Some(Shot::new(action, TURN_SECS));
        }
        let mut action = plan.action;
        match self.turn {
            Some(mut shot) if moving => {
                if shot.poll(s.action_done, dt) {
                    self.turn = None;
                } else {
                    self.turn = Some(shot);
                    action = shot.action;
                }
            }
            _ => self.turn = None,
        }
        Decision {
            action,
            steer: plan.steer,
            gait: plan.gait,
            facing: self.facing,
            look_at: plan.look_at,
        }
    }

    /// Сторона — по ходу движения (со сглаживанием и зазором против дрожи на
    /// вертикалях), а на месте — по взгляду.
    fn update_facing(&mut self, plan: &Plan, dt: f32, s: &Senses<'_>) {
        let moved = self.last_pos.map_or(Vec2::ZERO, |last| s.pos - last);
        self.last_pos = Some(s.pos);
        if matches!(plan.steer, Steer::Halt) {
            if let Some(at) = plan.look_at {
                let dx = at.x - s.pos.x;
                if dx.abs() > LOOK_DEAD_ZONE * s.scale {
                    self.facing = side(dx);
                }
            }
            self.travel = Vec2::new(side_sign(self.facing), 0.0);
            return;
        }
        if moved.length() > f32::EPSILON {
            let blend = 1.0 - (-dt * TRAVEL_RATE).exp();
            self.travel = self.travel.lerp(moved.normalized(), blend);
        }
        let direction = self.travel.normalized();
        if direction.x > FACE_SLACK {
            self.facing = Facing::Right;
        } else if direction.x < -FACE_SLACK {
            self.facing = Facing::Left;
        }
    }
}

fn side(dx: f32) -> Facing {
    if dx < 0.0 {
        Facing::Left
    } else {
        Facing::Right
    }
}

fn side_sign(facing: Facing) -> f32 {
    match facing {
        Facing::Left => -1.0,
        Facing::Right => 1.0,
    }
}

#[cfg(test)]
mod tests;
