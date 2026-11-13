//! Живой агент под консолью: исполняет выбранное, ведёт прогон, меняет тело.

use std::{
    collections::VecDeque,
    env,
    error::Error,
    io,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use ratatui::{DefaultTerminal, crossterm::event};
use ui_agent::{
    Agent, AgentConfig, AgentError, AgentEvent, Character, CursorGrabConfig, rig::RigKind,
};

use crate::app::{App, Note};
use crate::menu::{self, Item, Order, Step};
use crate::ui;

const POLL: Duration = Duration::from_millis(50);
// Флаги CreateProcess
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone)]
pub enum Source {
    Builtin(RigKind),
    Pack(PathBuf),
}

impl Source {
    fn load(&self) -> Result<Character, AgentError> {
        match self {
            Self::Builtin(kind) => Character::builtin(*kind),
            Self::Pack(dir) => Character::load_dir(dir),
        }
    }
}

/// Ключи запуска, которые повторяют этот персонаж в новом процессе.
fn relaunch_args(source: &Source, grab: bool) -> Vec<String> {
    let mut args = match source {
        Source::Builtin(kind) => vec!["--rig".into(), kind.name().into()],
        Source::Pack(dir) => vec!["--pack".into(), dir.display().to_string()],
    };
    if grab {
        args.push("--grab".into());
    }
    args
}

/// Та же программа без консоли; эта завершается, и её окно консоли закрывается.
fn detach(source: &Source, grab: bool) -> io::Result<()> {
    Command::new(env::current_exe()?)
        .args(relaunch_args(source, grab))
        .arg("--background")
        .creation_flags(DETACHED_PROCESS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// Та же программа в новом окне консоли. Через `start`: своих потоков у фона нет,
/// а унаследованные пустые сломали бы ввод-вывод консоли.
fn attach(source: &Source, grab: bool) -> io::Result<()> {
    let mut line = format!("/c start \"\" \"{}\"", env::current_exe()?.display());
    for arg in relaunch_args(source, grab) {
        line.push_str(&format!(" \"{arg}\""));
    }
    Command::new("cmd")
        .raw_arg(line)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

/// Фон: персонаж без консоли. Управление — его меню по правой кнопке; «Скрыть»
/// возвращает консоль, иначе до скрытого персонажа уже не добраться.
pub fn background(source: Source, grab: bool) -> Result<(), Box<dyn Error>> {
    let pilot = Pilot::launch(source.load()?, source, grab)?;
    loop {
        while let Some(event) = pilot.agent.try_event() {
            match event {
                AgentEvent::ExitRequested | AgentEvent::Stopped => {
                    pilot.agent.shutdown();
                    return Ok(());
                }
                AgentEvent::HiddenByUser => {
                    pilot.agent.shutdown();
                    return Ok(attach(&pilot.source, pilot.grab)?);
                }
                _ => {}
            }
        }
        thread::sleep(POLL);
    }
}

struct Pilot {
    agent: Agent,
    source: Source,
    grab: bool,
    /// Шаги прогона и время, когда каждый пора выполнить.
    queue: VecDeque<(Instant, Step)>,
}

pub fn run(
    terminal: &mut DefaultTerminal,
    source: Source,
    grab: bool,
) -> Result<(), Box<dyn Error>> {
    let character = source.load()?;
    let mut app = App::new(&character, grab);
    let mut pilot = Pilot::launch(character, source, grab)?;
    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        while let Some(event) = pilot.agent.try_event() {
            if !app.on_agent(&event) {
                pilot.agent.shutdown();
                return Ok(());
            }
        }
        pilot.run_queue(&mut app);
        if !event::poll(POLL)? {
            continue;
        }
        pilot = match app.on_input(event::read()?) {
            None => pilot,
            Some(Item::Quit) => break,
            Some(Item::Background) => {
                pilot.agent.shutdown();
                return Ok(detach(&pilot.source, pilot.grab)?);
            }
            Some(Item::Rig(kind)) if app.live.rig == Some(kind) => pilot,
            Some(Item::Rig(kind)) => {
                let grab = pilot.grab;
                pilot.relaunch(Source::Builtin(kind), grab, &mut app)?
            }
            Some(Item::Grab) => {
                let (source, grab) = (pilot.source.clone(), !pilot.grab);
                pilot.relaunch(source, grab, &mut app)?
            }
            Some(Item::Order(order)) => {
                pilot.execute(order, &mut app);
                pilot
            }
        };
    }
    pilot.agent.shutdown();
    Ok(())
}

impl Pilot {
    fn launch(character: Character, source: Source, grab: bool) -> Result<Self, AgentError> {
        let config = AgentConfig {
            cursor_grab: CursorGrabConfig {
                enabled: grab,
                ..CursorGrabConfig::default()
            },
            // Пакет для себя: озорные сценки включены
            naughty: true,
            ..AgentConfig::default()
        };
        let agent = Agent::start(character, config)?;
        agent.spawn()?;
        Ok(Self {
            agent,
            source,
            grab,
            queue: VecDeque::new(),
        })
    }

    /// Старые окна закрываются раньше, чем появятся новые.
    fn relaunch(self, source: Source, grab: bool, app: &mut App) -> Result<Self, AgentError> {
        self.agent.shutdown();
        let character = source.load()?;
        app.recast(&character, grab);
        Self::launch(character, source, grab)
    }

    fn execute(&mut self, order: Order, app: &mut App) {
        let sent = match order {
            Order::Step(step) => self.apply(step, app),
            Order::Run(run) => {
                self.schedule(menu::script(run, &app.cast), app);
                Ok(())
            }
            Order::Stop => {
                if app.progress.take().is_some() {
                    self.queue.clear();
                    app.note(Note::Command, "прогон остановлен".into());
                }
                Ok(())
            }
            Order::Spawn => self.agent.spawn(),
            Order::Despawn => self.agent.despawn(),
            Order::Pause => {
                app.live.paused = !app.live.paused;
                self.agent.pause(app.live.paused)
            }
            Order::Visible => {
                app.live.visible = !app.live.visible;
                self.agent.set_visible(app.live.visible)
            }
        };
        if let Err(error) = sent {
            app.note(Note::Error, error.to_string());
        }
    }

    fn apply(&self, step: Step, app: &mut App) -> Result<(), AgentError> {
        app.applied(&step);
        match step {
            Step::Play(action) => self.agent.play(&action),
            Step::Hold(Some(action)) => self.agent.hold(&action),
            Step::Hold(None) => self.agent.release(),
            Step::Stage(stunt) => self.agent.stage(stunt),
            Step::Status(status) => self.agent.set_status(status),
            Step::Mode(mode) => self.agent.set_behavior(mode),
            Step::Notify(note) => self.agent.notify(note),
            Step::Surprise => self.agent.surprise(),
        }
    }

    fn schedule(&mut self, steps: Vec<(Step, Duration)>, app: &mut App) {
        let mut at = Instant::now();
        self.queue.clear();
        for (step, hold) in steps {
            self.queue.push_back((at, step));
            at += hold;
        }
        app.progress = Some((0, self.queue.len()));
    }

    fn run_queue(&mut self, app: &mut App) {
        let now = Instant::now();
        while self.queue.front().is_some_and(|(at, _)| *at <= now) {
            let Some((_, step)) = self.queue.pop_front() else {
                break;
            };
            if let Err(error) = self.apply(step, app) {
                app.note(Note::Error, error.to_string());
            }
            if let Some((done, _)) = &mut app.progress {
                *done += 1;
            }
            if self.queue.is_empty() {
                app.progress = None;
                app.note(Note::Command, "прогон окончен".into());
            }
        }
    }
}
