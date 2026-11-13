//! Состояние экрана: что выбрано, что знаем о персонаже, журнал. Ввод превращается в пункт меню.

use std::collections::VecDeque;

use ratatui::{
    crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    layout::{Position, Rect},
    widgets::ListState,
};
use ui_agent::{AgentEvent, BehaviorMode, Character, rig::RigKind};

use crate::menu::{self, Cast, Item, Order, Section, Step};

const LOG_LINES: usize = 200;
const PAGE: usize = 10;

/// Что известно о живом персонаже: для отметок в меню и шапки.
pub struct Live {
    pub name: String,
    pub rig: Option<RigKind>,
    pub grab: bool,
    pub paused: bool,
    pub visible: bool,
    pub mode: BehaviorMode,
    pub held: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Note {
    Command,
    Event,
    Error,
}

/// Области, куда попал последний кадр: для щелчков мыши.
#[derive(Default)]
pub struct Areas {
    pub sidebar: Rect,
    pub list: Rect,
}

pub struct App {
    pub live: Live,
    pub cast: Cast,
    pub sections: Vec<Section>,
    pub section: ListState,
    /// Выбор в каждом разделе свой.
    pub lists: Vec<ListState>,
    pub log: VecDeque<(Note, String)>,
    /// Прогон: сделано шагов из скольких.
    pub progress: Option<(usize, usize)>,
    pub areas: Areas,
}

impl App {
    pub fn new(character: &Character, grab: bool) -> Self {
        let mut app = Self {
            live: Live {
                name: String::new(),
                rig: None,
                grab,
                paused: false,
                visible: true,
                mode: BehaviorMode::Wander,
                held: None,
            },
            cast: Cast::default(),
            sections: Vec::new(),
            section: ListState::default().with_selected(Some(0)),
            lists: Vec::new(),
            log: VecDeque::new(),
            progress: None,
            areas: Areas::default(),
        };
        app.recast(character, grab);
        app
    }

    /// Новый персонаж: меню строится заново, раздел остаётся.
    pub fn recast(&mut self, character: &Character, grab: bool) {
        self.cast = Cast::of(character, grab);
        self.sections = menu::sections(&self.cast);
        self.lists = self
            .sections
            .iter()
            .map(|_| ListState::default().with_selected(Some(0)))
            .collect();
        let rig = character.rig();
        self.live = Live {
            name: rig.map_or_else(|| character.id().to_owned(), |k| menu::rig_label(k).into()),
            rig,
            grab,
            paused: false,
            visible: true,
            mode: BehaviorMode::Wander,
            held: None,
        };
        self.progress = None;
        let name = self.live.name.clone();
        self.note(Note::Command, format!("на столе: {name}"));
    }

    pub fn note(&mut self, kind: Note, text: String) {
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back((kind, text));
    }

    /// Шаг отдан персонажу: журнал и отметки меню.
    pub fn applied(&mut self, step: &Step) {
        match step {
            Step::Mode(mode) => self.live.mode = *mode,
            Step::Hold(held) => self.live.held.clone_from(held),
            _ => {}
        }
        self.note(Note::Command, step.describe());
    }

    /// `false` — пользователь выбрал «Выход» в меню персонажа.
    pub fn on_agent(&mut self, event: &AgentEvent) -> bool {
        match event {
            AgentEvent::PauseToggled(paused) => self.live.paused = *paused,
            AgentEvent::HiddenByUser => self.live.visible = false,
            AgentEvent::ExitRequested => return false,
            _ => {}
        }
        self.note(Note::Event, describe(event));
        true
    }

    pub fn current(&self) -> usize {
        self.section.selected().unwrap_or(0)
    }

    pub fn on_input(&mut self, event: Event) -> Option<Item> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            _ => None,
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Option<Item> {
        let current = self.current();
        let list = &mut self.lists[current];
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Item::Quit);
            }
            KeyCode::Up => list.select_previous(),
            KeyCode::Down => list.select_next(),
            KeyCode::PageUp => list.scroll_up_by(PAGE as u16),
            KeyCode::PageDown => list.scroll_down_by(PAGE as u16),
            KeyCode::Home => list.select_first(),
            KeyCode::End => list.select_last(),
            KeyCode::Left | KeyCode::BackTab => self.turn(-1),
            KeyCode::Right | KeyCode::Tab => self.turn(1),
            KeyCode::Char(digit @ '1'..='9') => self.open(digit as usize - '1' as usize),
            KeyCode::Enter | KeyCode::Char(' ') => return self.chosen(),
            KeyCode::Char('p') => return Some(Item::Order(Order::Pause)),
            KeyCode::Char('h') => return Some(Item::Order(Order::Visible)),
            KeyCode::Char('r') => return Some(Item::Order(Order::Step(Step::Surprise))),
            KeyCode::Char('s') => return Some(Item::Order(Order::Stop)),
            KeyCode::Char('b') => return Some(Item::Background),
            KeyCode::Char('q') | KeyCode::Esc => return Some(Item::Quit),
            _ => {}
        }
        None
    }

    fn on_mouse(&mut self, mouse: MouseEvent) -> Option<Item> {
        let at = Position::new(mouse.column, mouse.row);
        let in_sidebar = self.areas.sidebar.contains(at);
        let in_list = self.areas.list.contains(at);
        let current = self.current();
        match mouse.kind {
            MouseEventKind::ScrollUp if in_sidebar => self.turn(-1),
            MouseEventKind::ScrollDown if in_sidebar => self.turn(1),
            MouseEventKind::ScrollUp if in_list => self.lists[current].select_previous(),
            MouseEventKind::ScrollDown if in_list => self.lists[current].select_next(),
            MouseEventKind::Down(MouseButton::Left) if in_sidebar => {
                let row = (at.y - self.areas.sidebar.y) as usize;
                self.open(self.section.offset() + row);
            }
            MouseEventKind::Down(MouseButton::Left) if in_list => {
                let list = &mut self.lists[current];
                let row = list.offset() + (at.y - self.areas.list.y) as usize;
                if row < self.sections[current].entries.len() {
                    list.select(Some(row));
                    return self.chosen();
                }
            }
            _ => {}
        }
        None
    }

    fn turn(&mut self, by: isize) {
        let count = self.sections.len() as isize;
        self.open((self.current() as isize + by).rem_euclid(count) as usize);
    }

    fn open(&mut self, section: usize) {
        if section < self.sections.len() {
            self.section.select(Some(section));
        }
    }

    fn chosen(&self) -> Option<Item> {
        let section = &self.sections[self.current()];
        let index = self.lists[self.current()].selected()?;
        section.entries.get(index).map(|entry| entry.item.clone())
    }
}

fn describe(event: &AgentEvent) -> String {
    match event {
        AgentEvent::Spawned => "появился".into(),
        AgentEvent::Despawned => "ушёл".into(),
        AgentEvent::Clicked => "по нему щёлкнули".into(),
        AgentEvent::Dropped(at) => format!("перенесён в {:.0}, {:.0}", at.x, at.y),
        AgentEvent::Played(action) => format!("доиграл {action}"),
        AgentEvent::StuntFinished(stunt) => format!("сценка {} окончена", stunt.name()),
        AgentEvent::NotificationClosed { by_user: true } => "карточку закрыли".into(),
        AgentEvent::NotificationClosed { by_user: false } => "карточка ушла по сроку".into(),
        AgentEvent::PauseToggled(true) => "пауза из меню".into(),
        AgentEvent::PauseToggled(false) => "продолжает из меню".into(),
        AgentEvent::HiddenByUser => "скрыт из меню".into(),
        AgentEvent::ExitRequested => "выход из меню".into(),
        AgentEvent::Stopped => "поток персонажа остановлен".into(),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEventState;

    use super::*;

    fn app() -> App {
        App::new(&Character::builtin(RigKind::Pig).unwrap(), false)
    }

    fn press(app: &mut App, code: KeyCode) -> Option<Item> {
        let key = KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        app.on_input(Event::Key(key))
    }

    #[test]
    fn enter_picks_selected_entry() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Down), None);
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            Some(Item::Rig(RigKind::Pig))
        );
    }

    #[test]
    fn sections_wrap_around() {
        let mut app = app();
        press(&mut app, KeyCode::Left);
        assert_eq!(app.current(), app.sections.len() - 1);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.current(), 0);
    }

    #[test]
    fn release_is_ignored() {
        let mut app = app();
        let key = KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::from(KeyCode::Char('q'))
        };
        assert_eq!(app.on_input(Event::Key(key)), None);
    }

    #[test]
    fn applied_step_marks_menu() {
        let mut app = app();
        app.applied(&Step::Mode(BehaviorMode::Sleep));
        app.applied(&Step::Hold(Some("celebrate".into())));
        assert_eq!(app.live.mode, BehaviorMode::Sleep);
        assert_eq!(app.live.held.as_deref(), Some("celebrate"));
        assert_eq!(app.log.back().unwrap().1, "держать celebrate");
    }

    #[test]
    fn agent_menu_drives_state() {
        let mut app = app();
        assert!(app.on_agent(&AgentEvent::PauseToggled(true)));
        assert!(app.on_agent(&AgentEvent::HiddenByUser));
        assert!(app.live.paused && !app.live.visible);
        assert!(!app.on_agent(&AgentEvent::ExitRequested));
    }

    #[test]
    fn log_is_bounded() {
        let mut app = app();
        for i in 0..LOG_LINES * 2 {
            app.note(Note::Event, i.to_string());
        }
        assert_eq!(app.log.len(), LOG_LINES);
    }
}
