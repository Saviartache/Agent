//! Спутник окна ветки: переводит занятость приложения в состояние персонажа и сам
//! развлекает его, пока работа тянется или ничего не происходит. Ветка зовёт три метода
//! из своего таймера и не знает ни о потоке персонажа, ни о Windows.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::command::{Notification, Status};
use crate::error::AgentError;
use crate::rng::Rng;

/// Чем занято приложение.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    /// Открывает сессию или готовится: персонаж думает.
    Connecting,
    /// Идёт запрос, выгрузка, пакет: персонаж занят.
    Working,
    /// Работы нет: персонаж гуляет сам.
    Idle,
    /// Последняя операция не удалась: разовая реакция, дальше гуляет.
    Failed,
}

/// Работа короче этого развлечений не получает, с.
const WORK_FIRST: [f32; 2] = [10.0, 18.0];
/// Пауза между выходками за работой, с.
const WORK_GAP: [f32; 2] = [18.0, 40.0];
/// Пауза между выходками без дела, с; свои сценки персонаж затевает и сам, реже.
const IDLE_GAP: [f32; 2] = [35.0, 90.0];

/// Что передать персонажу.
#[derive(Clone, Debug, PartialEq)]
enum Cue {
    Status(Status),
    Surprise,
}

/// Решения спутника без часов и ОС: время приходит в секундах от старта.
#[derive(Debug)]
struct Director {
    activity: Option<Activity>,
    /// Когда выкинуть следующую штуку; `None` — не время для выходок.
    next_surprise: Option<f32>,
    rng: Rng,
}

impl Director {
    fn new(seed: u64) -> Self {
        Self {
            activity: None,
            next_surprise: None,
            rng: Rng::new(seed),
        }
    }

    fn set(&mut self, activity: Activity, now: f32) -> Option<Cue> {
        let previous = self.activity.replace(activity);
        if previous == Some(activity) {
            return None;
        }
        self.next_surprise = match activity {
            Activity::Connecting => None,
            Activity::Working => Some(now + self.pick(WORK_FIRST)),
            Activity::Idle | Activity::Failed => Some(now + self.pick(IDLE_GAP)),
        };
        let status = match activity {
            Activity::Connecting => Status::Thinking,
            Activity::Working => Status::Busy,
            Activity::Failed => Status::Error,
            // Успех — только если до этого что-то делалось
            Activity::Idle => match previous {
                Some(Activity::Connecting | Activity::Working) => Status::Success,
                _ => Status::Normal,
            },
        };
        Some(Cue::Status(status))
    }

    fn tick(&mut self, now: f32) -> Option<Cue> {
        let due = self.next_surprise.filter(|at| now >= *at)?;
        let gap = match self.activity {
            Some(Activity::Working) => WORK_GAP,
            _ => IDLE_GAP,
        };
        self.next_surprise = Some(due.max(now) + self.pick(gap));
        Some(Cue::Surprise)
    }

    fn pick(&mut self, [min, max]: [f32; 2]) -> f32 {
        self.rng.range(min, max)
    }
}

/// Персонаж при окне ветки. Вне Windows и после «Выхода» из меню персонажа — пустышка:
/// методы ничего не делают, ветка зовёт их одинаково.
pub struct Companion {
    director: Director,
    started: Instant,
    #[cfg(windows)]
    agent: Option<crate::agent::Agent>,
}

impl Companion {
    /// Запускает и выпускает персонажа; вне Windows возвращает пустышку.
    pub fn start() -> Result<Self, AgentError> {
        #[cfg(windows)]
        {
            let character = crate::assets::Character::builtin(crate::rig::RigKind::Pig)?;
            let agent =
                crate::agent::Agent::start(character, crate::config::AgentConfig::default())?;
            agent.spawn()?;
            Ok(Self {
                agent: Some(agent),
                ..Self::disabled()
            })
        }
        #[cfg(not(windows))]
        Ok(Self::disabled())
    }

    /// Без персонажа: на случай отказа запуска.
    pub fn disabled() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos() as u64;
        Self {
            director: Director::new(seed),
            started: Instant::now(),
            #[cfg(windows)]
            agent: None,
        }
    }

    /// Сменить занятость; тот же вид повторно ничего не шлёт, поэтому звать можно на каждом тике.
    pub fn set(&mut self, activity: Activity) {
        let now = self.now();
        if let Some(cue) = self.director.set(activity, now) {
            self.send(cue);
        }
    }

    /// Карточка с итогом операции; реакцию задаёт `set`.
    pub fn notify(&mut self, notification: Notification) {
        #[cfg(windows)]
        self.deliver(|agent| agent.notify(notification));
        #[cfg(not(windows))]
        let _ = notification;
    }

    /// Звать из таймера окна (раз в 100–500 мс): выходки по расписанию и события персонажа.
    pub fn tick(&mut self) {
        let now = self.now();
        if let Some(cue) = self.director.tick(now) {
            self.send(cue);
        }
        #[cfg(windows)]
        self.drain();
    }

    fn now(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    fn send(&mut self, cue: Cue) {
        #[cfg(windows)]
        self.deliver(|agent| match cue {
            Cue::Status(status) => agent.set_status(status),
            Cue::Surprise => agent.surprise(),
        });
        #[cfg(not(windows))]
        let _ = cue;
    }

    /// Персонаж — украшение: полная очередь теряет команду, остановленный поток гасит спутника.
    #[cfg(windows)]
    fn deliver(&mut self, command: impl FnOnce(&crate::agent::Agent) -> Result<(), AgentError>) {
        let Some(agent) = &self.agent else { return };
        if let Err(AgentError::Stopped) = command(agent) {
            self.agent = None;
        }
    }

    /// «Выход» в меню персонажа убирает только его, окно ветки живёт дальше.
    #[cfg(windows)]
    fn drain(&mut self) {
        let Some(agent) = &self.agent else { return };
        let mut exit = false;
        while let Some(event) = agent.try_event() {
            exit |= matches!(
                event,
                crate::event::AgentEvent::ExitRequested | crate::event::AgentEvent::Stopped
            );
        }
        if exit && let Some(agent) = self.agent.take() {
            agent.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn director() -> Director {
        Director::new(7)
    }

    #[test]
    fn activities_map_to_statuses() {
        let mut d = director();
        assert_eq!(
            d.set(Activity::Idle, 0.0),
            Some(Cue::Status(Status::Normal))
        );
        assert_eq!(
            d.set(Activity::Connecting, 1.0),
            Some(Cue::Status(Status::Thinking))
        );
        assert_eq!(
            d.set(Activity::Idle, 2.0),
            Some(Cue::Status(Status::Success))
        );
        assert_eq!(
            d.set(Activity::Working, 3.0),
            Some(Cue::Status(Status::Busy))
        );
        assert_eq!(
            d.set(Activity::Failed, 4.0),
            Some(Cue::Status(Status::Error))
        );
        assert_eq!(
            d.set(Activity::Idle, 5.0),
            Some(Cue::Status(Status::Normal))
        );
    }

    #[test]
    fn repeating_an_activity_sends_nothing() {
        let mut d = director();
        d.set(Activity::Working, 0.0);
        assert_eq!(d.set(Activity::Working, 1.0), None);
    }

    #[test]
    fn short_work_gets_no_surprise() {
        let mut d = director();
        d.set(Activity::Working, 0.0);
        assert_eq!(d.tick(WORK_FIRST[0] - 0.1), None);
        d.set(Activity::Idle, WORK_FIRST[0] - 0.1);
        assert_eq!(d.tick(WORK_FIRST[0] + 1.0), None);
    }

    #[test]
    fn long_work_is_spiced_up_regularly() {
        let mut d = director();
        d.set(Activity::Working, 0.0);
        let surprises = (0..600)
            .filter(|s| d.tick(*s as f32) == Some(Cue::Surprise))
            .count();
        let most = 1 + ((600.0 - WORK_FIRST[0]) / WORK_GAP[0]) as usize;
        let least = ((600.0 - WORK_FIRST[1]) / WORK_GAP[1]) as usize;
        assert!((least..=most).contains(&surprises), "{surprises}");
    }

    #[test]
    fn idle_gets_surprises_and_connecting_does_not() {
        let mut d = director();
        d.set(Activity::Connecting, 0.0);
        assert!((0..300).all(|s| d.tick(s as f32).is_none()));
        d.set(Activity::Idle, 300.0);
        assert!((300..300 + IDLE_GAP[1] as i32 + 1).any(|s| d.tick(s as f32).is_some()));
    }

    #[test]
    fn a_late_tick_does_not_burst() {
        let mut d = director();
        d.set(Activity::Idle, 0.0);
        assert_eq!(d.tick(10_000.0), Some(Cue::Surprise));
        assert_eq!(d.tick(10_000.0), None);
    }

    #[test]
    fn disabled_companion_accepts_everything() {
        let mut companion = Companion::disabled();
        companion.set(Activity::Working);
        companion.notify(Notification::success("Готово"));
        companion.tick();
        companion.set(Activity::Idle);
    }
}
