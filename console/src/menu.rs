//! Что можно выбрать: разделы меню, их пункты и прогоны.

use std::time::Duration;

use ui_agent::{BehaviorMode, Character, Notification, Status, Stunt, Tone, rig::RigKind};

/// Пауза между шагами прогона: анимация успевает доиграть.
const STEP_GAP: Duration = Duration::from_millis(2600);
/// Столько держится состояние или режим, прежде чем прогон идёт дальше.
const HOLD: Duration = Duration::from_secs(4);

pub const STATUSES: [(&str, Status); 6] = [
    ("обычное", Status::Normal),
    ("думает", Status::Thinking),
    ("занят", Status::Busy),
    ("успех", Status::Success),
    ("предупреждение", Status::Warning),
    ("ошибка", Status::Error),
];

pub const MODES: [(&str, BehaviorMode); 5] = [
    ("гуляет сам", BehaviorMode::Wander),
    ("стоит на месте", BehaviorMode::Stay),
    ("идёт за курсором", BehaviorMode::FollowCursor),
    ("гонится за курсором", BehaviorMode::ChaseCursor),
    ("спит", BehaviorMode::Sleep),
];

const TONES: [(&str, Tone); 4] = [
    ("сведения", Tone::Info),
    ("успех", Tone::Success),
    ("предупреждение", Tone::Warning),
    ("ошибка", Tone::Error),
];

pub fn label<T: PartialEq>(table: &[(&'static str, T)], value: &T) -> &'static str {
    table
        .iter()
        .find(|(_, item)| item == value)
        .map_or("?", |(name, _)| name)
}

pub fn rig_label(kind: RigKind) -> &'static str {
    match kind {
        RigKind::Goose => "гусь",
        RigKind::Pig => "свинка",
        RigKind::Penguin => "пингвин",
    }
}

/// Команда персонажу, которую можно поставить в прогон.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Play(String),
    Hold(Option<String>),
    Stage(Stunt),
    Status(Status),
    Mode(BehaviorMode),
    Notify(Notification),
    Surprise,
}

impl Step {
    pub fn describe(&self) -> String {
        match self {
            Self::Play(action) => format!("сыграть {action}"),
            Self::Hold(Some(action)) => format!("держать {action}"),
            Self::Hold(None) => "отпустить".into(),
            Self::Stage(stunt) => format!("сценка {}", stunt.name()),
            Self::Status(status) => format!("состояние: {}", label(&STATUSES, status)),
            Self::Mode(mode) => format!("режим: {}", label(&MODES, mode)),
            Self::Notify(note) => format!("карточка: {}", note.title),
            Self::Surprise => "что-нибудь случайное".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// Все действия, состояния, режимы, карточки и сценки.
    All,
    /// Думает → занят → успех с карточкой.
    Demo,
}

/// Команда персонажу, которую исполняет живой агент.
#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    Step(Step),
    Run(Run),
    Stop,
    Spawn,
    Despawn,
    Pause,
    Visible,
}

/// Выбор пользователя. Тело и перехват перезапускают агента, остальное — приказ ему.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Rig(RigKind),
    Grab,
    /// Консоль закрывается, персонаж живёт дальше отдельным процессом.
    Background,
    Quit,
    Order(Order),
}

pub struct Entry {
    pub label: String,
    pub item: Item,
}

pub struct Section {
    pub title: &'static str,
    pub entries: Vec<Entry>,
}

/// Что умеет текущий персонаж.
#[derive(Default)]
pub struct Cast {
    pub actions: Vec<String>,
    pub stunts: Vec<Stunt>,
}

impl Cast {
    /// Кража курсора — только с перехватом.
    pub fn of(character: &Character, grab: bool) -> Self {
        Self {
            actions: character.action_names().map(str::to_owned).collect(),
            stunts: Stunt::roster(character.rig())
                .filter(|stunt| grab || *stunt != Stunt::CursorHeist)
                .collect(),
        }
    }
}

fn entry(label: impl Into<String>, item: Item) -> Entry {
    Entry {
        label: label.into(),
        item,
    }
}

fn step(label: impl Into<String>, step: Step) -> Entry {
    entry(label, Item::Order(Order::Step(step)))
}

fn order(label: &str, order: Order) -> Entry {
    entry(label, Item::Order(order))
}

pub fn sections(cast: &Cast) -> Vec<Section> {
    let rigs = RigKind::ALL.map(|kind| entry(rig_label(kind), Item::Rig(kind)));
    let plays = cast.actions.iter().map(|a| step(a, Step::Play(a.clone())));
    let holds = cast
        .actions
        .iter()
        .map(|a| step(a, Step::Hold(Some(a.clone()))));
    vec![
        Section {
            title: "Персонаж",
            entries: rigs
                .into_iter()
                .chain([entry("перехват курсора", Item::Grab)])
                .collect(),
        },
        Section {
            title: "Действия",
            entries: plays.collect(),
        },
        Section {
            title: "Держать",
            entries: [step("отпустить", Step::Hold(None))]
                .into_iter()
                .chain(holds)
                .collect(),
        },
        Section {
            title: "Сценки",
            entries: cast
                .stunts
                .iter()
                .map(|stunt| step(stunt.name(), Step::Stage(*stunt)))
                .collect(),
        },
        Section {
            title: "Состояние",
            entries: STATUSES
                .iter()
                .map(|(name, status)| step(*name, Step::Status(*status)))
                .collect(),
        },
        Section {
            title: "Режим",
            entries: MODES
                .iter()
                .map(|(name, mode)| step(*name, Step::Mode(*mode)))
                .collect(),
        },
        Section {
            title: "Карточки",
            entries: TONES
                .iter()
                .map(|(name, tone)| step(*name, Step::Notify(card(*tone, name))))
                .collect(),
        },
        Section {
            title: "Прогоны",
            entries: vec![
                order("всё подряд", Order::Run(Run::All)),
                order("думает → занят → успех", Order::Run(Run::Demo)),
                step("что-нибудь случайное", Step::Surprise),
                order("остановить прогон", Order::Stop),
            ],
        },
        Section {
            title: "Окно",
            entries: vec![
                order("появиться", Order::Spawn),
                order("уйти", Order::Despawn),
                order("пауза", Order::Pause),
                order("скрыт", Order::Visible),
                entry("в фон, без консоли", Item::Background),
                entry("выход", Item::Quit),
            ],
        },
    ]
}

fn card(tone: Tone, name: &str) -> Notification {
    Notification::new(tone, format!("Карточка: {name}")).body("Проверка")
}

/// Шаги прогона, каждый со своей паузой после себя.
pub fn script(run: Run, cast: &Cast) -> Vec<(Step, Duration)> {
    match run {
        Run::All => all(cast),
        Run::Demo => vec![
            (Step::Status(Status::Thinking), Duration::from_secs(3)),
            (Step::Status(Status::Busy), HOLD),
            (Step::Status(Status::Success), Duration::ZERO),
            (
                Step::Notify(Notification::success("Выгрузка завершена").body("Реестр сохранён")),
                STEP_GAP,
            ),
            (Step::Play("wave".into()), STEP_GAP),
        ],
    }
}

fn all(cast: &Cast) -> Vec<(Step, Duration)> {
    let plays = cast
        .actions
        .iter()
        .map(|a| (Step::Play(a.clone()), STEP_GAP));
    let statuses = STATUSES[1..].iter().map(|(_, s)| (Step::Status(*s), HOLD));
    let modes = MODES[1..].iter().map(|(_, m)| (Step::Mode(*m), HOLD));
    let notes = TONES
        .iter()
        .map(|(name, tone)| (Step::Notify(card(*tone, name)), STEP_GAP));
    let stunts = cast.stunts.iter().map(|s| (Step::Stage(*s), HOLD * 3));
    plays
        .chain(statuses)
        .chain([(Step::Status(Status::Normal), STEP_GAP)])
        .chain(modes)
        .chain([(Step::Mode(BehaviorMode::Wander), STEP_GAP)])
        .chain(notes)
        .chain([
            (Step::Hold(Some("celebrate".into())), HOLD),
            (Step::Hold(None), STEP_GAP),
        ])
        .chain(stunts)
        .chain([(Step::Surprise, STEP_GAP)])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pig() -> Cast {
        Cast::of(&Character::builtin(RigKind::Pig).unwrap(), false)
    }

    #[test]
    fn heist_needs_grab() {
        let character = Character::builtin(RigKind::Goose).unwrap();
        assert!(
            !Cast::of(&character, false)
                .stunts
                .contains(&Stunt::CursorHeist)
        );
        assert!(
            Cast::of(&character, true)
                .stunts
                .contains(&Stunt::CursorHeist)
        );
    }

    #[test]
    fn every_section_has_entries() {
        let cast = pig();
        for section in sections(&cast) {
            assert!(!section.entries.is_empty(), "{}", section.title);
        }
    }

    #[test]
    fn full_run_covers_cast() {
        let cast = pig();
        let steps = script(Run::All, &cast);
        let fixed = STATUSES.len() + MODES.len() + TONES.len() + 3;
        assert_eq!(steps.len(), cast.actions.len() + cast.stunts.len() + fixed);
    }
}
