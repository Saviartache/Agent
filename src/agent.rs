//! Публичные дескрипторы персонажа: команды в поток, события из потока, без блокировок.
//! `Agent` владеет потоком; `AgentHandle` — его копируемая «ручка» для любого места программы.

use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};

use crate::assets::Character;
use crate::command::{BehaviorMode, Command, Notification, Status, Stunt};
use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::math::Vec2;
use crate::platform::pump::Waker;
use crate::runtime;

const QUEUE: usize = 64;

/// Команды персонажу из любого потока. Клонируется дёшево; все методы возвращаются
/// сразу: `Busy` — очередь полна и команда отброшена, `Stopped` — персонаж остановлен.
#[derive(Clone)]
pub struct AgentHandle {
    commands: SyncSender<Command>,
    waker: Waker,
    /// Поднят владельцем: копии ручки не должны держать поток живым.
    stopped: Arc<AtomicBool>,
}

impl AgentHandle {
    pub fn spawn(&self) -> Result<(), AgentError> {
        self.send(Command::Spawn(None))
    }

    pub fn spawn_at(&self, at: Vec2) -> Result<(), AgentError> {
        self.send(Command::Spawn(Some(at)))
    }

    pub fn despawn(&self) -> Result<(), AgentError> {
        self.send(Command::Despawn)
    }

    pub fn set_behavior(&self, mode: BehaviorMode) -> Result<(), AgentError> {
        self.send(Command::SetBehavior(mode))
    }

    /// Состояние приложения: `Thinking`/`Busy` держатся, остальные отыгрываются разово.
    pub fn set_status(&self, status: Status) -> Result<(), AgentError> {
        self.send(Command::SetStatus(status))
    }

    /// Разовое действие по имени из набора; неизвестное имя игнорируется.
    pub fn play(&self, action: &str) -> Result<(), AgentError> {
        self.send(Command::Play(action.to_owned()))
    }

    /// Своё состояние: действие из набора держится, пока не придёт `release`.
    pub fn hold(&self, action: &str) -> Result<(), AgentError> {
        self.send(Command::Hold(Some(action.to_owned())))
    }

    pub fn release(&self) -> Result<(), AgentError> {
        self.send(Command::Hold(None))
    }

    /// Разыграть сценку с предметами на рабочем столе.
    pub fn stage(&self, stunt: Stunt) -> Result<(), AgentError> {
        self.send(Command::Stage(stunt))
    }

    /// Что-нибудь случайное: разовое действие набора или сценка.
    pub fn surprise(&self) -> Result<(), AgentError> {
        self.send(Command::Surprise)
    }

    pub fn move_to(&self, to: Vec2) -> Result<(), AgentError> {
        self.send(Command::MoveTo(to))
    }

    pub fn notify(&self, notification: Notification) -> Result<(), AgentError> {
        self.send(Command::Notify(notification))
    }

    /// Состояние и карточка одним вызовом: итог операции приложения.
    pub fn report(&self, status: Status, notification: Notification) -> Result<(), AgentError> {
        self.set_status(status)?;
        self.notify(notification)
    }

    pub fn pause(&self, paused: bool) -> Result<(), AgentError> {
        self.send(Command::Pause(paused))
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), AgentError> {
        self.send(Command::SetVisible(visible))
    }

    /// Любая команда как есть: для вызывающих, которые получают её данными (`ffi/`).
    pub fn send(&self, command: Command) -> Result<(), AgentError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(AgentError::Stopped);
        }
        let sent = match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(AgentError::Busy),
            Err(TrySendError::Disconnected(_)) => return Err(AgentError::Stopped),
        };
        self.waker.wake();
        sent
    }
}

/// Персонаж на рабочем столе. Поток персонажа живёт, пока этот дескриптор не остановлен
/// (`shutdown`) или не отброшен; команды доступны и через него, и через копии `handle()`.
pub struct Agent {
    handle: AgentHandle,
    events: Receiver<AgentEvent>,
    thread: Option<JoinHandle<()>>,
}

impl Agent {
    /// Запускает поток и ждёт, пока он создаст окна; ошибка создания возвращается сюда.
    pub fn start(character: Character, config: AgentConfig) -> Result<Self, AgentError> {
        let (command_tx, command_rx) = sync_channel(QUEUE);
        let (event_tx, event_rx) = sync_channel(QUEUE);
        let (ready_tx, ready_rx) = sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopped);
        let thread = thread::Builder::new()
            .name("ui-agent".into())
            .spawn(move || runtime::run(character, config, command_rx, event_tx, ready_tx, flag))?;
        match ready_rx.recv() {
            Ok(Ok(waker)) => Ok(Self {
                handle: AgentHandle {
                    commands: command_tx,
                    waker,
                    stopped,
                },
                events: event_rx,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            // Поток умер, не ответив
            Err(_) => {
                let _ = thread.join();
                Err(AgentError::Stopped)
            }
        }
    }

    /// Ручка для других потоков и частей программы; события остаются у владельца.
    pub fn handle(&self) -> AgentHandle {
        self.handle.clone()
    }

    /// Следующее событие, если оно уже есть.
    pub fn try_event(&self) -> Option<AgentEvent> {
        self.events.try_recv().ok()
    }

    /// Закрывает окна и ждёт завершения потока.
    pub fn shutdown(mut self) {
        self.stop();
    }

    /// Повторный вызов ничего не делает.
    fn stop(&mut self) {
        // Флаг, а не команда: очередь может быть полна, а копии ручки держат канал открытым
        self.handle.stopped.store(true, Ordering::Release);
        self.handle.waker.wake();
        if let Some(thread) = self.thread.take() {
            // Паника потока не должна ронять приложение
            let _ = thread.join();
        }
    }
}

impl Deref for Agent {
    type Target = AgentHandle;

    fn deref(&self) -> &AgentHandle {
        &self.handle
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn wait_for(agent: &Agent, wanted: AgentEvent) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match agent.try_event() {
                Some(event) if event == wanted => return true,
                Some(_) => {}
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
        false
    }

    fn start() -> Agent {
        let character = Character::builtin(crate::rig::RigKind::Pig).unwrap();
        Agent::start(character, AgentConfig::default()).unwrap()
    }

    #[test]
    fn handle_can_be_shared_between_threads() {
        fn shareable<T: Clone + Send + Sync + 'static>() {}
        fn movable<T: Send + 'static>() {}
        shareable::<AgentHandle>();
        movable::<Agent>();
    }

    #[test]
    fn spawns_and_shuts_down_cleanly() {
        let agent = start();
        agent.spawn().unwrap();
        assert!(wait_for(&agent, AgentEvent::Spawned));
        agent
            .report(Status::Busy, Notification::info("Проверка"))
            .unwrap();
        agent.stage(Stunt::LeafPile).unwrap();
        agent.hold("think").unwrap();
        agent.release().unwrap();
        agent.surprise().unwrap();
        agent.shutdown();
    }

    #[test]
    fn dropping_without_shutdown_joins_the_thread() {
        let agent = start();
        agent.spawn().unwrap();
        drop(agent);
    }

    #[test]
    fn flooding_never_blocks_and_shutdown_still_gets_through() {
        let mut agent = start();
        for _ in 0..10 * QUEUE {
            let sent = agent.play("wave");
            assert!(matches!(sent, Ok(()) | Err(AgentError::Busy)));
        }
        agent.stop();
        assert!(agent.thread.is_none());
        agent.stop();
    }

    #[test]
    fn handles_command_from_other_threads_and_die_with_the_owner() {
        let mut agent = start();
        let handle = agent.handle();
        let worker = thread::spawn(move || {
            handle.spawn().unwrap();
            handle
        });
        let handle = worker.join().unwrap();
        assert!(wait_for(&agent, AgentEvent::Spawned));
        agent.stop();
        assert!(matches!(handle.spawn(), Err(AgentError::Stopped)));
        assert!(matches!(agent.pause(true), Err(AgentError::Stopped)));
        assert!(wait_for(&agent, AgentEvent::Stopped));
    }
}
