//! Пакет персонажа: живая консоль, где тело и команды выбираются из меню.
//! `ui-agent-console [--rig goose|pig|dachshund] [--pack <папка>] [--grab] [--background]`
//! `ui-agent-console [--rig goose|pig|elephant] [--pack <папка>] [--grab] [--background]`
//! `ui-agent-console [--rig goose|pig|raccoon] [--pack <папка>] [--grab] [--background]`
//! `ui-agent-console [--rig goose|pig|penguin] [--pack <папка>] [--grab] [--background]`

// Меню и кадр проверяются тестами на любой машине; агент есть только в Windows
#[cfg(any(windows, test))]
mod app;
#[cfg(any(windows, test))]
mod menu;
#[cfg(windows)]
mod pilot;
#[cfg(any(windows, test))]
mod ui;

#[cfg(not(windows))]
fn main() {
    println!("Консоль персонажа работает только в Windows.");
}

#[cfg(windows)]
fn main() {
    if let Err(error) = start() {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn start() -> Result<(), Box<dyn std::error::Error>> {
    use ratatui::crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
    };

    let (source, grab, background) = options()?;
    if background {
        return pilot::background(source, grab);
    }
    let mut terminal = ratatui::init();
    execute!(std::io::stdout(), EnableMouseCapture)?;
    let result = pilot::run(&mut terminal, source, grab);
    // Терминал возвращается и при ошибке: её текст должен быть виден
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(windows)]
fn options() -> Result<(pilot::Source, bool, bool), String> {
    use ui_agent::rig::RigKind;

    let mut source = pilot::Source::Builtin(RigKind::Pig);
    let mut grab = false;
    let mut background = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pack" => {
                source = pilot::Source::Pack(args.next().ok_or("--pack требует папку")?.into())
            }
            "--rig" => {
                let name = args.next().ok_or("--rig требует имя: goose, pig или dachshund")?;
                let name = args.next().ok_or("--rig требует имя: goose, pig или elephant")?;
                let name = args
                    .next()
                    .ok_or("--rig требует имя: goose, pig или raccoon")?;
                let name = args.next().ok_or("--rig требует имя: goose, pig или penguin")?;
                let kind = RigKind::from_name(&name).ok_or(format!("нет такого тела: {name}"))?;
                source = pilot::Source::Builtin(kind);
            }
            "--grab" => grab = true,
            "--background" => background = true,
            other => return Err(format!("неизвестный аргумент: {other}")),
        }
    }
    Ok((source, grab, background))
}
