//! Поток персонажа: единственный владелец окон, симуляции и цикла ожидания.
//! Без таймера он спит в системе до команды или ввода; опроса и `sleep` нет.

mod cards;
mod stage;

use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use cards::Cards;
use stage::Stage;

use crate::assets::Character;
use crate::command::Command;
use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::math::{Bounds, Vec2};
use crate::platform::{
    self, cursor, monitors,
    pump::{Pump, Wake, Waker},
    shell,
    window::{self, WindowEvent},
};
use crate::sim::{Frame, Pointer, Sim};

const MIN_FPS: u32 = 10;
const MAX_FPS: u32 = 120;
/// Раз в столько секунд проверяются полноэкранные приложения и DPI.
const CHECK_SECS: f32 = 1.0;
/// Скрыт полноэкранным приложением или на паузе: опрос раз в секунду.
const BUSY_POLL: Duration = Duration::from_secs(1);
const ERROR_NO_MONITORS: u32 = 1461;

const MENU_PAUSE: u32 = 1;
const MENU_HIDE: u32 = 2;
const MENU_EXIT: u32 = 3;

struct Settings {
    period: Duration,
    context_menu: bool,
    hide_in_fullscreen: bool,
    grab: bool,
}

struct Runtime {
    sim: Sim,
    settings: Settings,
    bounds: Bounds,
    dpi: u32,
    commands: Receiver<Command>,
    events: SyncSender<AgentEvent>,
    window_events: Vec<WindowEvent>,
    outgoing: Vec<AgentEvent>,
    stage: Stage,
    cards: Cards,
    pump: Pump,
    clock: Instant,
    /// Секунды с последней проверки полноэкранных приложений и DPI.
    since_check: f32,
    /// Скрыт командой приложения или меню.
    visible: bool,
    /// Полноэкранное приложение, презентация или заблокированный экран.
    busy: bool,
    /// Таймер выключен; при включении часы начинаются заново.
    idle: bool,
    stopping: bool,
}

fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}

/// Тело потока; возвращает, когда приложение попросило остановиться.
pub(crate) fn run(
    character: Character,
    config: AgentConfig,
    commands: Receiver<Command>,
    events: SyncSender<AgentEvent>,
    ready: SyncSender<Result<Waker, AgentError>>,
    stopped: Arc<AtomicBool>,
) {
    platform::init_thread();
    let (mut runtime, waker) = match Runtime::new(character, &config, commands, events) {
        Ok(started) => started,
        Err(error) => {
            // Приложение ждёт ответа; если его уже нет, сказать некому
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(waker)).is_err() {
        return;
    }
    runtime.run_loop(&stopped);
    let events = runtime.close();
    // Приложение могло уйти или не читать события: остановку сообщаем по возможности
    let _ = events.try_send(AgentEvent::Stopped);
}

impl Runtime {
    fn new(
        character: Character,
        config: &AgentConfig,
        commands: Receiver<Command>,
        events: SyncSender<AgentEvent>,
    ) -> Result<(Self, Waker), AgentError> {
        let (pump, waker) = Pump::new()?;
        let stage = Stage::new()?;
        let bounds = monitors::bounds().ok_or(AgentError::Platform {
            call: "EnumDisplayMonitors",
            code: ERROR_NO_MONITORS,
        })?;
        let dpi = monitors::dpi_at(bounds.hull().center());
        let fps = config.fps.clamp(MIN_FPS, MAX_FPS);
        let runtime = Self {
            sim: Sim::new(character, config, bounds.clone(), dpi, clock_seed()),
            settings: Settings {
                period: Duration::from_secs_f64(1.0 / f64::from(fps)),
                context_menu: config.context_menu,
                hide_in_fullscreen: config.hide_in_fullscreen,
                grab: config.cursor_grab.enabled,
            },
            bounds,
            dpi,
            commands,
            events,
            window_events: Vec::new(),
            outgoing: Vec::new(),
            stage,
            cards: Cards::default(),
            pump,
            clock: Instant::now(),
            since_check: CHECK_SECS,
            visible: true,
            busy: false,
            idle: true,
            stopping: false,
        };
        Ok((runtime, waker))
    }

    /// Освобождает окна и таймер до того, как приложению сообщат об остановке.
    fn close(self) -> SyncSender<AgentEvent> {
        self.events
    }

    /// `stopped` поднимает владелец дескриптора: очередь команд для этого ненадёжна.
    fn run_loop(&mut self, stopped: &AtomicBool) {
        while !self.stopping {
            let wake = self.pump.wait();
            if matches!(wake, Wake::Closed) || stopped.load(Ordering::Acquire) {
                break;
            }
            self.drain_commands();
            self.drain_window_events();
            if matches!(wake, Wake::Tick) && !self.stopping {
                self.tick();
            }
            self.forward_events();
            self.sync_timer();
        }
    }

    // --- команды приложения ---

    fn drain_commands(&mut self) {
        loop {
            match self.commands.try_recv() {
                Ok(command) => self.command(command),
                Err(TryRecvError::Empty) => return,
                // Приложение ушло, не попрощавшись: то же, что Shutdown
                Err(TryRecvError::Disconnected) => {
                    self.stopping = true;
                    return;
                }
            }
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::SetVisible(visible) => self.set_visible(visible),
            Command::Shutdown => self.stopping = true,
            other => self.sim.command(other),
        }
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.check_busy();
        }
        self.apply_permission();
    }

    // --- события окон ---

    fn drain_window_events(&mut self) {
        let mut events = mem::take(&mut self.window_events);
        window::drain_events(&mut events);
        for event in events.drain(..) {
            self.on_window_event(event);
        }
        self.window_events = events;
    }

    fn on_window_event(&mut self, event: WindowEvent) {
        let character = self.stage.character_id();
        match event {
            WindowEvent::PointerDown { layer, at } if layer == character => {
                self.sim.pointer(Pointer::Down(at));
            }
            WindowEvent::PointerUp { layer, at } if layer == character => {
                self.sim.pointer(Pointer::Up(at));
            }
            WindowEvent::PointerUp { layer, .. } => {
                if let Some(item) = self.cards.item_at(layer) {
                    self.sim.dismiss_item(item);
                }
            }
            WindowEvent::PointerCancel { layer } if layer == character => {
                self.sim.pointer(Pointer::Cancel);
            }
            WindowEvent::ContextMenu { layer, at } if layer == character => {
                if self.settings.context_menu {
                    self.show_menu(at);
                }
            }
            WindowEvent::DisplayChanged => self.refresh_display(),
            WindowEvent::CloseRequested => self.emit(AgentEvent::ExitRequested),
            WindowEvent::PointerDown { .. }
            | WindowEvent::PointerCancel { .. }
            | WindowEvent::ContextMenu { .. } => {}
        }
    }

    fn show_menu(&mut self, at: Vec2) {
        let pause = if self.sim.paused() {
            "Продолжить"
        } else {
            "Пауза"
        };
        let items = [
            (MENU_PAUSE, pause),
            (MENU_HIDE, "Скрыть"),
            (MENU_EXIT, "Выход"),
        ];
        let chosen = self.stage.popup_menu(at, &items);
        // Меню модальное: время в нём не считается
        self.clock = Instant::now();
        match chosen {
            Some(MENU_PAUSE) => self.toggle_pause(),
            Some(MENU_HIDE) => {
                self.set_visible(false);
                self.emit(AgentEvent::HiddenByUser);
            }
            Some(MENU_EXIT) => self.emit(AgentEvent::ExitRequested),
            _ => {}
        }
    }

    fn toggle_pause(&mut self) {
        let paused = !self.sim.paused();
        self.sim.command(Command::Pause(paused));
        self.emit(AgentEvent::PauseToggled(paused));
    }

    // --- кадр ---

    fn tick(&mut self) {
        let dt = self.elapsed();
        self.housekeeping(dt);
        if !self.permitted() {
            return;
        }
        let user_input = self.settings.grab && cursor::user_input_active();
        let frame = Frame {
            dt,
            cursor: cursor::position(),
            user_input,
            permitted: true,
        };
        if let Some(to) = self.sim.step(&frame) {
            cursor::set_position(to);
        }
        self.stage.draw(&self.sim);
        self.cards.sync(self.sim.items(), self.sim.ui_scale());
    }

    fn elapsed(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.clock).as_secs_f32();
        self.clock = now;
        dt
    }

    fn housekeeping(&mut self, dt: f32) {
        self.since_check += dt;
        if self.since_check < CHECK_SECS {
            return;
        }
        self.since_check = 0.0;
        self.check_busy();
        self.refresh_dpi();
    }

    fn check_busy(&mut self) {
        let busy = self.settings.hide_in_fullscreen && shell::user_busy();
        if busy != self.busy {
            self.busy = busy;
            self.apply_permission();
        }
    }

    fn permitted(&self) -> bool {
        self.visible && !self.busy
    }

    /// Без разрешения окна прячутся, а симуляция отпускает курсор и стоит.
    fn apply_permission(&mut self) {
        if self.permitted() {
            return;
        }
        self.stage.hide();
        self.cards.hide();
        let frame = Frame {
            dt: 0.0,
            cursor: None,
            user_input: false,
            permitted: false,
        };
        self.sim.step(&frame);
    }

    // --- мониторы ---

    fn refresh_display(&mut self) {
        if let Some(bounds) = monitors::bounds() {
            self.bounds = bounds;
        }
        self.dpi = monitors::dpi_at(self.sim.position());
        self.sim.set_display(self.bounds.clone(), self.dpi);
    }

    /// Персонаж мог зайти на монитор с другим масштабом.
    fn refresh_dpi(&mut self) {
        let dpi = monitors::dpi_at(self.sim.position());
        if dpi != self.dpi {
            self.dpi = dpi;
            self.sim.set_display(self.bounds.clone(), dpi);
        }
    }

    // --- таймер и события наружу ---

    fn interval(&self) -> Option<Duration> {
        if !self.visible || self.sim.is_dormant() {
            None
        // На паузе кадров нет, но полноэкранное приложение замечать нужно
        } else if self.busy || self.sim.paused() {
            Some(BUSY_POLL)
        } else {
            Some(self.settings.period)
        }
    }

    fn sync_timer(&mut self) {
        let interval = self.interval();
        if self.idle && interval.is_some() {
            self.clock = Instant::now();
            // Пока таймер спал, мог смениться монитор персонажа или экран занят
            self.since_check = CHECK_SECS;
        }
        self.idle = interval.is_none();
        self.pump.set_interval(interval);
    }

    /// Очередь приложения не ждём: переполненная теряет событие.
    fn emit(&self, event: AgentEvent) {
        let _ = self.events.try_send(event);
    }

    fn forward_events(&mut self) {
        self.sim.drain_events(&mut self.outgoing);
        for event in self.outgoing.drain(..) {
            let _ = self.events.try_send(event);
        }
    }
}
