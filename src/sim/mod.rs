//! Симуляция персонажа: мозг, движение, анимация и эффекты в одном детерминированном шаге.
//! Без ОС и часов — время, курсор и ввод приходят в `Frame`, результат читает отрисовка.

mod anim;
mod cards;
mod condition;
mod drive;
mod effects;
mod pointer;
mod rig;
mod step;
mod stunt;
#[cfg(test)]
mod tests;
mod trail;

use anim::Animator;
use cards::Carry;
use condition::Conditions;
use drive::Drive;
pub use pointer::Pointer;
use pointer::Press;
use stunt::Act;
use trail::Trail;

use crate::action::ActionId;
use crate::assets::Character;
use crate::behavior::{Brain, BrainEvent, CursorGrab, Directive, Facing, Intent, Permissions};
use crate::command::Command;
use crate::config::AgentConfig;
use crate::effects::{
    footprints::Footprints,
    items::{Closed, Items},
    leaves::Leaves,
    mud::Mud,
    particles::Particles,
    props::Props,
};
use crate::event::AgentEvent;
use crate::math::{Bounds, Rect, Vec2};
use crate::motion::{Body, Mover};
use crate::render::{
    Sprite,
    scene::{self, CharacterView, Effects, SpriteView},
};
use crate::rig::{Antic, Creature, Footfall, GROUND_SQUASH, Pose};
use crate::rng::Rng;

/// Длиннее кадр не считаем: после остановки потока или сна машины персонаж не прыгает.
const MAX_FRAME_SECS: f32 = 0.25;
const SUBSTEP_SECS: f32 = 1.0 / 60.0;
const EVENT_CAP: usize = 64;
const FOOTPRINT_CAP: usize = 32;
const FOOTPRINT_SECS: f32 = 4.5;
/// Логические px.
const FOOTPRINT_SPACING: f32 = 26.0;
const HALF_GAUGE: f32 = 5.0;
const PARTICLE_CAP: usize = 96;
const LEAF_CAP: usize = 96;
const PUDDLE_CAP: usize = 4;
/// Грязная дорожка длинная и держится дольше обычных следов.
const MUD_PRINT_CAP: usize = 128;
const MUD_PRINT_SECS: f32 = 16.0;
const PROP_CAP: usize = 24;
/// Каракули клювом: плотная гаснущая линия.
const SCRIBBLE_CAP: usize = 600;
const SCRIBBLE_SECS: f32 = 6.0;
/// Логические px.
const SCRIBBLE_SPACING: f32 = 2.5;
/// Медленнее этого персонаж не шагает и следов не оставляет, логические px/с.
const MOVING_SPEED: f32 = 4.0;
/// Сглаживание скорости курсора, 1/с.
const CURSOR_SMOOTHING: f32 = 20.0;
/// Дальше этого от любого монитора персонажа переносят сразу, логические px.
const STRANDED: f32 = 200.0;
const MIN_UI_SCALE: f32 = 0.25;
const MAX_UI_SCALE: f32 = 8.0;
/// Разводит поток случайных чисел движения и мозга.
const RNG_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;

/// Один кадр внешнего мира.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub dt: f32,
    pub cursor: Option<Vec2>,
    /// Нажата кнопка мыши или Esc.
    pub user_input: bool,
    /// `false`: скрыт, полноэкранное приложение или остановка — симуляция не идёт.
    pub permitted: bool,
}

#[derive(Clone, Copy)]
struct Settings {
    footprints: bool,
    particles: bool,
    draggable: bool,
    grab: bool,
    stunts: bool,
    naughty: bool,
    start: Option<Vec2>,
}

pub struct Sim {
    character: Character,
    settings: Settings,
    user_scale: f32,
    /// Множитель логических расстояний.
    ui: f32,
    /// Множитель спрайтов.
    sprite: f32,
    bounds: Bounds,
    brain: Brain,
    mover: Mover,
    body: Body,
    rng: Rng,
    anim: Animator,
    /// Есть у персонажа с процедурным телом; у спрайтового — `None`.
    rig: Option<Creature>,
    footfalls: Vec<Footfall>,
    footprints: Footprints,
    particles: Particles,
    leaves: Leaves,
    mud: Mud,
    mud_prints: Footprints,
    /// Состояния после сценок: грязь, оглушение, проплешина.
    cond: Conditions,
    props: Props,
    scribble: Footprints,
    /// Позы гусят; их ведёт сценка.
    flock: Vec<Pose>,
    /// Накладка позы от сценки и состояний; задаётся заново каждый шаг.
    antic: Antic,
    /// Сценка ведёт тело сама.
    drive: Option<Drive>,
    /// Ведомое тело упёрлось в край: нормаль внутрь экрана; только на этом шаге.
    bumped: Option<Vec2>,
    /// По персонажу щёлкнули с прошлого кадра.
    clicked: bool,
    /// Время жизни симуляции, с.
    clock: f32,
    /// Курсор на этом шаге.
    cursor: Option<Vec2>,
    trail: Trail,
    items: Items,
    /// Идущая сценка.
    act: Option<Act>,
    /// Через сколько секунд свободной прогулки персонаж сам затеет сценку.
    stunt_in: f32,
    stunt_gap: [f32; 2],
    /// Не раньше этих часов (`clock`) курсор снова напугает персонажа.
    startle_ready: f32,
    /// Действия для «чего-нибудь случайного».
    ambient: Vec<ActionId>,
    grab: CursorGrab,
    carrying: Vec<Carry>,
    press: Option<Press>,
    paused: bool,
    /// `Spawn` принят, но мозг ещё не вышел из `Absent`.
    spawn_pending: bool,
    /// Действие, которое приложение не заказывало (перенос карточки).
    silent: Option<ActionId>,
    facing: Facing,
    arrived: bool,
    cursor_prev: Option<Vec2>,
    cursor_vel: Vec2,
    events: Vec<AgentEvent>,
    brain_events: Vec<BrainEvent>,
    closed: Vec<Closed>,
}

fn ui_scale(user: f32, dpi: u32) -> f32 {
    let scale = user * dpi as f32 / 96.0;
    if scale.is_finite() {
        scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE)
    } else {
        1.0
    }
}

fn clean_dt(dt: f32) -> f32 {
    if dt > 0.0 {
        dt.min(MAX_FRAME_SECS)
    } else {
        0.0
    }
}

fn finite(point: Vec2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

/// Переполнение вытесняет самое старое событие.
fn push_event(events: &mut Vec<AgentEvent>, event: AgentEvent) {
    if events.len() == EVENT_CAP {
        events.remove(0);
    }
    events.push(event);
}

impl Sim {
    pub fn new(
        character: Character,
        config: &AgentConfig,
        bounds: Bounds,
        dpi: u32,
        seed: u64,
    ) -> Self {
        let seed = config.seed.unwrap_or(seed);
        let ui = ui_scale(config.scale, dpi);
        let permissions = Permissions {
            cursor_interest: config.cursor_interest,
            click_reaction: config.click_reaction,
        };
        let body = Body::at(bounds.hull().center());
        let rig = character
            .rig()
            .map(|kind| Creature::new(kind, body.pos, body.heading, ui * character.scale()));
        let mut rng = Rng::new(seed ^ RNG_SALT);
        let stunt_gap = character.behavior().stunt_gap_secs;
        Self {
            leaves: Leaves::new(LEAF_CAP),
            mud: Mud::new(PUDDLE_CAP),
            mud_prints: Footprints::new(MUD_PRINT_CAP, MUD_PRINT_SECS, FOOTPRINT_SPACING * ui),
            cond: Conditions::default(),
            props: Props::new(PROP_CAP),
            scribble: Footprints::new(SCRIBBLE_CAP, SCRIBBLE_SECS, SCRIBBLE_SPACING * ui),
            flock: Vec::new(),
            antic: Antic::default(),
            drive: None,
            bumped: None,
            clicked: false,
            clock: 0.0,
            cursor: None,
            trail: Trail::default(),
            act: None,
            stunt_in: rng.range(stunt_gap[0].max(0.0), stunt_gap[1].max(stunt_gap[0])),
            stunt_gap,
            startle_ready: 0.0,
            ambient: stunt::ambient(&character),
            brain: Brain::new(
                character.behavior().clone(),
                *character.actions(),
                permissions,
                seed,
            ),
            anim: Animator::new(&character),
            sprite: ui * character.scale(),
            settings: Settings {
                footprints: config.footprints,
                particles: config.particles,
                draggable: config.draggable,
                grab: config.cursor_grab.enabled,
                stunts: config.stunts,
                naughty: config.naughty,
                start: config.start,
            },
            user_scale: config.scale,
            ui,
            body,
            bounds,
            mover: Mover::new(),
            rng,
            rig,
            footfalls: Vec::new(),
            footprints: Footprints::new(FOOTPRINT_CAP, FOOTPRINT_SECS, FOOTPRINT_SPACING * ui),
            particles: Particles::new(PARTICLE_CAP),
            items: Items::new(config.max_items),
            grab: CursorGrab::new(config.cursor_grab.clone()),
            carrying: Vec::new(),
            press: None,
            paused: false,
            spawn_pending: false,
            silent: None,
            facing: Facing::Right,
            arrived: false,
            cursor_prev: None,
            cursor_vel: Vec2::ZERO,
            events: Vec::with_capacity(EVENT_CAP),
            brain_events: Vec::new(),
            closed: Vec::new(),
            character,
        }
    }

    /// `SetVisible` и `Shutdown` принадлежат потоку окон и здесь игнорируются.
    pub fn command(&mut self, command: Command) {
        match command {
            Command::Spawn(at) => self.spawn(at),
            Command::Despawn => {
                self.press = None;
                self.cancel_stunt();
                self.brain.push(Directive::Despawn);
            }
            Command::SetBehavior(mode) => self.brain.push(Directive::Mode(mode)),
            Command::Play(name) => self.play(&name),
            // Неизвестное имя игнорируется, как и у `Play`
            Command::Hold(Some(name)) => {
                if let Some(action) = self.character.action_id(&name) {
                    self.brain.push(Directive::Hold(Some(action)));
                }
            }
            Command::Hold(None) => self.brain.push(Directive::Hold(None)),
            Command::Stage(stunt) => {
                self.stage(stunt);
            }
            Command::Surprise => self.surprise(),
            Command::MoveTo(to) => {
                let to = self.bounds.clamp(to, self.edge_margin());
                self.brain.push(Directive::MoveTo(to));
            }
            Command::SetStatus(status) => self.brain.push(Directive::Status(status)),
            Command::Notify(notification) => self.notify(notification),
            Command::Pause(paused) => {
                self.paused = paused;
                // Удержание курсора не переживает паузу
                self.grab.release();
            }
            Command::SetVisible(_) | Command::Shutdown => {}
        }
    }

    pub fn set_display(&mut self, bounds: Bounds, dpi: u32) {
        let ui = ui_scale(self.user_scale, dpi);
        if (ui - self.ui).abs() > f32::EPSILON {
            self.ui = ui;
            self.sprite = ui * self.character.scale();
            self.footprints =
                Footprints::new(FOOTPRINT_CAP, FOOTPRINT_SECS, FOOTPRINT_SPACING * ui);
            self.mud_prints =
                Footprints::new(MUD_PRINT_CAP, MUD_PRINT_SECS, FOOTPRINT_SPACING * ui);
            self.scribble = Footprints::new(SCRIBBLE_CAP, SCRIBBLE_SECS, SCRIBBLE_SPACING * ui);
            self.reset_rig();
        }
        self.bounds = bounds;
        let nearest = self.bounds.nearest(self.body.pos);
        if nearest.clamp(self.body.pos, 0.0).distance(self.body.pos) > STRANDED * self.ui {
            // Монитор отключили: переносим сразу, пустоту не пересекают
            self.body.pos = self.bounds.clamp(self.body.pos, self.edge_margin());
            self.body.vel = Vec2::ZERO;
            self.reset_rig();
        }
        let bounds = &self.bounds;
        self.items.relocate(|point| bounds.clamp(point, 0.0));
    }

    pub fn drain_events(&mut self, out: &mut Vec<AgentEvent>) {
        out.append(&mut self.events);
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Персонажа нет и эффектов нет: таймеру кадров делать нечего.
    pub fn is_dormant(&self) -> bool {
        !self.is_present()
            && self.footprints.is_empty()
            && self.particles.is_empty()
            && self.leaves.is_empty()
            && self.mud.is_empty()
            && self.mud_prints.is_empty()
            && self.props.is_empty()
            && self.scribble.is_empty()
            && self.flock.is_empty()
            && self.items.is_empty()
    }

    /// Всё, что рисуется в окне эффектов.
    pub fn effects(&self) -> Effects<'_> {
        Effects {
            footprints: &self.footprints,
            print_radius: self
                .character
                .rig()
                .map_or(scene::PRINT_RADIUS, |kind| kind.print_radius()),
            particles: &self.particles,
            leaves: &self.leaves,
            mud: &self.mud,
            mud_prints: &self.mud_prints,
            props: &self.props,
            scribble: &self.scribble,
            flock: &self.flock,
        }
    }

    pub fn position(&self) -> Vec2 {
        self.body.pos
    }

    pub fn ui_scale(&self) -> f32 {
        self.ui
    }

    /// `None`, пока персонажа нет на экране.
    pub fn character(&self) -> Option<CharacterView<'_>> {
        if self.brain.intent() == Intent::Absent {
            return None;
        }
        if let Some(rig) = &self.rig {
            return Some(CharacterView::Creature {
                pose: rig.pose(),
                extent: Creature::extent(self.sprite),
            });
        }
        let clip = self.character.clip(self.anim.clip());
        let frame = clip.frames.get(self.anim.frame())?;
        let sheet = self.character.sheet(frame.sheet);
        Some(CharacterView::Sprite(SpriteView {
            sheet: frame.sheet,
            sprite: Sprite {
                pixels: &sheet.pixels,
                stride: sheet.width as usize,
                rect: frame.rect,
            },
            anchor: clip.anchor,
            offset: frame.offset,
            pos: self.body.pos,
            flip: self.facing == Facing::Left,
            scale: self.sprite,
        }))
    }

    /// Монитор, за краем которого персонаж прячется; рисуется только то, что на нём.
    /// `None` — персонаж весь на экране.
    pub fn character_clip(&self) -> Option<Rect> {
        let hiding = self.drive.is_some_and(|drive| drive.overhang > 0.0);
        (hiding || !self.bounds.contains(self.body.pos)).then(|| self.bounds.nearest(self.body.pos))
    }

    pub fn footprints(&self) -> &Footprints {
        &self.footprints
    }

    pub fn particles(&self) -> &Particles {
        &self.particles
    }

    pub fn items(&self) -> &Items {
        &self.items
    }

    // --- состояние ---

    fn is_present(&self) -> bool {
        self.spawn_pending || self.brain.intent() != Intent::Absent
    }

    /// Живёт целиком: не появляется и не уходит.
    fn alive(&self) -> bool {
        !matches!(
            self.brain.intent(),
            Intent::Absent | Intent::Spawning | Intent::Despawning
        )
    }

    fn edge_margin(&self) -> f32 {
        self.character.motion().edge_margin * self.ui
    }

    /// Кончик клюва или рыла; у спрайта — над головой, на уровне верха кадра.
    fn head(&self) -> Vec2 {
        if let Some(rig) = &self.rig {
            return rig.head();
        }
        let anchor = self.character.clip(self.anim.clip()).anchor;
        self.body.pos - Vec2::new(0.0, anchor.y * self.sprite)
    }

    /// Высота кончика клюва или рыла над землёй, физические px; у спрайта — 0.
    fn muzzle_height(&self) -> f32 {
        self.rig
            .as_ref()
            .map_or(0.0, |rig| rig.pose().muzzle_tip().z)
    }

    /// Точка на земле под кончиком клюва или рыла.
    fn muzzle_ground(&self) -> Vec2 {
        self.head() + Vec2::new(0.0, self.muzzle_height())
    }

    /// Куда смотреть и где стоять, чтобы кончик клюва или рыла с вылетом `reach`
    /// (design px от точки под телом) пришёлся на точку земли `at`.
    fn reach_for(&self, at: Vec2, reach: f32) -> (Vec2, Vec2) {
        let to = at - self.body.pos;
        // Глубина на экране сжата: смотреть надо круче, чем видно
        let face = Vec2::new(to.x, to.y / GROUND_SQUASH).normalized();
        let ahead = Vec2::new(face.x, face.y * GROUND_SQUASH);
        (face, at - ahead * (reach * self.sprite))
    }

    // --- команды ---

    fn spawn(&mut self, at: Option<Vec2>) {
        if self.is_present() {
            return;
        }
        let first = self.bounds.areas()[0];
        let default = Vec2::new(first.center().x, first.max.y);
        let at = at.filter(|point| finite(*point));
        let at = at.or(self.settings.start).unwrap_or(default);
        self.body = Body::at(self.bounds.clamp(at, self.edge_margin()));
        // Появляется чистым и бодрым
        self.cond = Conditions::default();
        self.reset_rig();
        self.arrived = false;
        self.spawn_pending = true;
        self.brain.push(Directive::Spawn);
    }

    /// Неизвестное имя игнорируется.
    fn play(&mut self, name: &str) {
        if let Some(action) = self.character.action_id(name) {
            self.silent = None;
            self.brain.push(Directive::Perform(action));
        }
    }
}
