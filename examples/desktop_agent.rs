//! Демонстрация: персонаж на рабочем столе Windows, управляемый из консоли.
//! `cargo run -p ui-agent --example desktop_agent -- [--pack <папка>] [--rig goose|pig|dachshund] [--grab]`
//! `cargo run -p ui-agent --example desktop_agent -- [--pack <папка>] [--rig goose|pig|elephant] [--grab]`
//! `cargo run -p ui-agent --example desktop_agent -- [--pack <папка>] [--rig goose|pig|raccoon] [--grab]`
//! `cargo run -p ui-agent --example desktop_agent -- [--pack <папка>] [--rig goose|pig|penguin] [--grab]`
//! Команды консоли — `help`; ещё есть меню по правой кнопке.

#[cfg(not(windows))]
fn main() {
    println!("Демонстрация работает только в Windows.");
}

#[cfg(windows)]
fn main() {
    if let Err(error) = demo::run() {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod demo {
    use std::{
        collections::VecDeque,
        error::Error,
        io::{self, BufRead},
        path::PathBuf,
        sync::mpsc::{self, RecvTimeoutError},
        thread,
        time::{Duration, Instant},
    };

    use ui_agent::{
        Agent, AgentConfig, AgentEvent, BehaviorMode, Character, CursorGrabConfig, Notification,
        Status, Stunt, Tone, Vec2, rig::RigKind,
    };

    const POLL: Duration = Duration::from_millis(50);
    /// Пауза между шагами прогона: анимация успевает доиграть.
    const STEP_GAP: Duration = Duration::from_millis(2600);
    /// Столько держится состояние или режим, прежде чем прогон идёт дальше.
    const HOLD: Duration = Duration::from_secs(4);

    const HELP: &str = "\
Команды:
  help                     эта справка
  actions                  имена действий набора
  play <действие>          разовое действие (например: play celebrate)
  hold <действие>          держать действие как своё состояние; release — отпустить
  stunts                   сценки персонажа: общие и своего вида
  stunt <сценка>           разыграть сценку (например: stunt zoomies; cursor_heist — с --grab)
  random                   что-нибудь случайное: действие или сценка
  status <состояние>       normal | thinking | busy | success | warning | error
  mode <режим>             wander | stay | follow | chase | sleep
  notify <тон> <текст>     info | success | warning | error; текст до конца строки
  move <x> <y>             идти в точку экрана
  spawn / despawn          появиться / уйти
  all                      прогнать все действия, состояния, режимы, карточки и сценки
  demo                     сценарий: думает → занят → успех с карточкой
  stop                     прервать прогон
  p / h / q                пауза / скрыть-показать / выход";

    const STATUSES: [(&str, Status); 6] = [
        ("normal", Status::Normal),
        ("thinking", Status::Thinking),
        ("busy", Status::Busy),
        ("success", Status::Success),
        ("warning", Status::Warning),
        ("error", Status::Error),
    ];

    const MODES: [(&str, BehaviorMode); 5] = [
        ("wander", BehaviorMode::Wander),
        ("stay", BehaviorMode::Stay),
        ("follow", BehaviorMode::FollowCursor),
        ("chase", BehaviorMode::ChaseCursor),
        ("sleep", BehaviorMode::Sleep),
    ];

    const TONES: [(&str, Tone); 4] = [
        ("info", Tone::Info),
        ("success", Tone::Success),
        ("warning", Tone::Warning),
        ("error", Tone::Error),
    ];

    fn find<T: Copy>(table: &[(&str, T)], name: &str) -> Option<T> {
        table
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    }

    fn names<T>(table: &[(&str, T)]) -> String {
        table
            .iter()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Один шаг прогона.
    #[derive(Clone)]
    enum Step {
        Play(String),
        Status(Status),
        Mode(BehaviorMode),
        Notify(Notification),
        Hold(Option<String>),
        Stage(Stunt),
        Surprise,
    }

    struct Options {
        pack: Option<PathBuf>,
        rig: RigKind,
        grab: bool,
    }

    fn options() -> Result<Options, String> {
        let mut options = Options {
            pack: None,
            rig: RigKind::Pig,
            grab: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--pack" => options.pack = Some(args.next().ok_or("--pack требует папку")?.into()),
                "--rig" => {
                    let name = args
                        .next()
                        .ok_or("--rig требует имя: goose, pig или dachshund")?;
                        .ok_or("--rig требует имя: goose, pig или elephant")?;
                        .ok_or("--rig требует имя: goose, pig или raccoon")?;
                        .ok_or("--rig требует имя: goose, pig или penguin")?;
                    options.rig =
                        RigKind::from_name(&name).ok_or(format!("нет такого тела: {name}"))?;
                }
                "--grab" => options.grab = true,
                other => return Err(format!("неизвестный аргумент: {other}")),
            }
        }
        Ok(options)
    }

    /// Строки консоли приходят из отдельного потока: чтение stdin блокирует.
    fn console() -> mpsc::Receiver<String> {
        let (lines, input) = mpsc::channel();
        thread::spawn(move || {
            for line in io::stdin().lock().lines().map_while(Result::ok) {
                if lines.send(line).is_err() {
                    return;
                }
            }
        });
        input
    }

    struct Demo {
        agent: Agent,
        actions: Vec<String>,
        /// Сценки персонажа; кража курсора — только с `--grab`.
        stunts: Vec<Stunt>,
        /// Шаги прогона и время, когда каждый пора выполнить.
        queue: VecDeque<(Instant, Step)>,
        paused: bool,
        visible: bool,
    }

    impl Demo {
        fn stunt_names(&self) -> String {
            let names: Vec<_> = self.stunts.iter().map(|stunt| stunt.name()).collect();
            names.join(" | ")
        }

        fn on_event(&mut self, event: &AgentEvent) -> bool {
            println!("событие: {event:?}");
            match event {
                AgentEvent::PauseToggled(paused) => self.paused = *paused,
                AgentEvent::HiddenByUser => self.visible = false,
                AgentEvent::ExitRequested => return false,
                _ => {}
            }
            true
        }

        fn apply(&self, step: Step) -> Result<(), Box<dyn Error>> {
            match step {
                Step::Play(action) => {
                    println!("▸ play {action}");
                    self.agent.play(&action)?;
                }
                Step::Status(status) => {
                    println!("▸ status {status:?}");
                    self.agent.set_status(status)?;
                }
                Step::Mode(mode) => {
                    println!("▸ mode {mode:?}");
                    self.agent.set_behavior(mode)?;
                }
                Step::Notify(note) => {
                    println!("▸ notify {:?}: {}", note.tone, note.title);
                    self.agent.notify(note)?;
                }
                Step::Hold(Some(action)) => {
                    println!("▸ hold {action}");
                    self.agent.hold(&action)?;
                }
                Step::Hold(None) => {
                    println!("▸ release");
                    self.agent.release()?;
                }
                Step::Stage(stunt) => {
                    println!("▸ stunt {stunt:?}");
                    self.agent.stage(stunt)?;
                }
                Step::Surprise => {
                    println!("▸ random");
                    self.agent.surprise()?;
                }
            }
            Ok(())
        }

        /// Ставит шаги в очередь друг за другом, каждый со своей паузой после себя.
        fn schedule(&mut self, steps: impl IntoIterator<Item = (Step, Duration)>) {
            let mut at = Instant::now();
            self.queue.clear();
            for (step, hold) in steps {
                self.queue.push_back((at, step));
                at += hold;
            }
        }

        /// Все действия набора, затем состояния, режимы и карточки всех тонов.
        fn schedule_all(&mut self) {
            let plays = self.actions.clone().into_iter().map(Step::Play);
            let statuses = STATUSES[1..]
                .iter()
                .map(|(_, status)| Step::Status(*status));
            let modes = MODES[1..].iter().map(|(_, mode)| Step::Mode(*mode));
            let notes = TONES.iter().map(|(name, tone)| {
                Step::Notify(Notification::new(*tone, format!("Карточка: {name}")).body("Проверка"))
            });
            let steps: Vec<_> = plays
                .map(|step| (step, STEP_GAP))
                .chain(statuses.map(|step| (step, HOLD)))
                .chain([(Step::Status(Status::Normal), STEP_GAP)])
                .chain(modes.map(|step| (step, HOLD)))
                .chain([(Step::Mode(BehaviorMode::Wander), STEP_GAP)])
                .chain(notes.map(|step| (step, STEP_GAP)))
                .chain([
                    (Step::Hold(Some("celebrate".into())), HOLD),
                    (Step::Hold(None), STEP_GAP),
                ])
                .chain(
                    self.stunts
                        .iter()
                        .map(|stunt| (Step::Stage(*stunt), HOLD * 3)),
                )
                .chain([(Step::Surprise, STEP_GAP)])
                .collect();
            println!("прогон: шагов {}, прервать — stop", steps.len());
            self.schedule(steps);
        }

        fn schedule_demo(&mut self) {
            let note = Notification::success("Выгрузка завершена").body("Реестр сохранён");
            self.schedule([
                (Step::Status(Status::Thinking), Duration::from_secs(3)),
                (Step::Status(Status::Busy), HOLD),
                (Step::Status(Status::Success), Duration::ZERO),
                (Step::Notify(note), STEP_GAP),
                (Step::Play("wave".into()), STEP_GAP),
            ]);
        }

        fn run_queue(&mut self) -> Result<(), Box<dyn Error>> {
            while self
                .queue
                .front()
                .is_some_and(|(at, _)| *at <= Instant::now())
            {
                if let Some((_, step)) = self.queue.pop_front() {
                    self.apply(step)?;
                }
                if self.queue.is_empty() {
                    println!("прогон окончен");
                }
            }
            Ok(())
        }

        /// `Ok(false)` — выход.
        fn on_line(&mut self, line: &str) -> Result<bool, Box<dyn Error>> {
            let line = line.trim();
            let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
            let rest = rest.trim();
            match command {
                "" => {}
                "help" | "?" => println!("{HELP}"),
                "actions" => println!("{}", self.actions.join(" ")),
                "play" if self.actions.iter().any(|action| action == rest) => {
                    self.apply(Step::Play(rest.into()))?;
                }
                "play" => println!("нет такого действия; список — actions"),
                "status" => match find(&STATUSES, rest) {
                    Some(status) => self.apply(Step::Status(status))?,
                    None => println!("status {}", names(&STATUSES)),
                },
                "mode" => match find(&MODES, rest) {
                    Some(mode) => self.apply(Step::Mode(mode))?,
                    None => println!("mode {}", names(&MODES)),
                },
                "notify" => {
                    let (tone, text) = rest.split_once(' ').unwrap_or((rest, ""));
                    match find(&TONES, tone) {
                        Some(tone) if !text.trim().is_empty() => {
                            self.apply(Step::Notify(Notification::new(tone, text.trim())))?;
                        }
                        _ => println!("notify {} <текст>", names(&TONES)),
                    }
                }
                "move" => {
                    let mut numbers = rest.split_whitespace().map(str::parse::<f32>);
                    match (numbers.next(), numbers.next()) {
                        (Some(Ok(x)), Some(Ok(y))) => self.agent.move_to(Vec2::new(x, y))?,
                        _ => println!("move <x> <y>"),
                    }
                }
                "spawn" => self.agent.spawn()?,
                "despawn" => self.agent.despawn()?,
                "hold" if self.actions.iter().any(|action| action == rest) => {
                    self.apply(Step::Hold(Some(rest.into())))?;
                }
                "hold" => println!("нет такого действия; список — actions"),
                "release" => self.apply(Step::Hold(None))?,
                "stunts" => println!("{}", self.stunt_names()),
                "stunt" => match Stunt::from_name(rest).filter(|s| self.stunts.contains(s)) {
                    Some(stunt) => self.apply(Step::Stage(stunt))?,
                    None => println!("stunt {}", self.stunt_names()),
                },
                "random" => self.apply(Step::Surprise)?,
                "all" => self.schedule_all(),
                "demo" => self.schedule_demo(),
                "stop" => self.queue.clear(),
                "p" => {
                    self.paused = !self.paused;
                    self.agent.pause(self.paused)?;
                }
                "h" => {
                    self.visible = !self.visible;
                    self.agent.set_visible(self.visible)?;
                }
                "q" | "exit" => return Ok(false),
                _ => println!("не понял «{line}»; справка — help"),
            }
            Ok(true)
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let options = options()?;
        let character = match &options.pack {
            Some(dir) => Character::load_dir(dir)?,
            None => Character::builtin(options.rig)?,
        };
        let actions = character.action_names().map(str::to_owned).collect();
        let stunts = Stunt::roster(character.rig())
            .filter(|stunt| options.grab || *stunt != Stunt::CursorHeist)
            .collect();
        let config = AgentConfig {
            cursor_grab: CursorGrabConfig {
                enabled: options.grab,
                ..CursorGrabConfig::default()
            },
            // Демо для себя: озорные сценки включены
            naughty: true,
            ..AgentConfig::default()
        };
        println!("{HELP}");
        let mut demo = Demo {
            agent: Agent::start(character, config)?,
            actions,
            stunts,
            queue: VecDeque::new(),
            paused: false,
            visible: true,
        };
        demo.agent.spawn()?;
        let input = console();
        let mut running = true;
        let mut failure = None;
        while running && failure.is_none() {
            while let Some(event) = demo.agent.try_event() {
                running &= demo.on_event(&event);
            }
            if let Err(error) = demo.run_queue() {
                failure = Some(error);
            }
            match input.recv_timeout(POLL) {
                Ok(line) => match demo.on_line(&line) {
                    Ok(keep) => running &= keep,
                    Err(error) => failure = Some(error),
                },
                Err(RecvTimeoutError::Timeout) => {}
                // Консоли нет (stdin закрыт): работаем до выхода из меню
                Err(RecvTimeoutError::Disconnected) => thread::sleep(POLL),
            }
        }
        demo.agent.shutdown();
        failure.map_or(Ok(()), Err)
    }
}
