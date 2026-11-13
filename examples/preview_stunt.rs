//! Кадры сценки для проверки глазами: настоящая симуляция, эффекты и персонаж вместе.
//! `cargo run -p ui-agent --example preview_stunt -- <goose|pig|raccoon> <сценка> <out.png> [ключи]`
//! `--secs N` — сколько снимать (8), `--every N` — шаг кадров (0.4),
//! `--cursor x,y` — где курсор от персонажа (180,-40), `--orbit` — курсор ходит кругом,
//! `--ui N` — масштаб экрана (1.5), `--crop w,h` — кадр вокруг персонажа (400,280). Печатает события и итог сценки.

use std::{error::Error, fs::File, io::BufWriter, path::PathBuf};

use ui_agent::{
    AgentConfig, AgentEvent, Character, Command, CursorGrabConfig, Stunt,
    math::{Bounds, Rect, Vec2},
    render::{
        Canvas, creature,
        scene::{self, CharacterView},
    },
    rig::RigKind,
    sim::{Frame, Sim},
};

const SCREEN: [f32; 2] = [1600.0, 900.0];
/// Кадр вокруг персонажа, логические px.
const CROP: Vec2 = Vec2::new(400.0, 280.0);
const COLUMNS: usize = 4;
const BACKGROUND: [u32; 3] = [118, 124, 134];
const DT: f32 = 1.0 / 60.0;
const ORBIT_RADIUS: f32 = 150.0;

struct Options {
    kind: RigKind,
    stunt: Stunt,
    path: PathBuf,
    secs: f32,
    every: f32,
    cursor: Vec2,
    orbit: bool,
    ui: f32,
    crop: Vec2,
}

fn parse() -> Result<Options, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let kind = args
        .next()
        .and_then(|name| RigKind::from_name(&name))
        .ok_or("первый аргумент — тело: goose, pig или raccoon")?;
    let stunt = args
        .next()
        .and_then(|name| Stunt::from_name(&name))
        .ok_or("второй аргумент — имя сценки")?;
    let path = args.next().ok_or("третий аргумент — путь к PNG")?.into();
    let mut options = Options {
        kind,
        stunt,
        path,
        secs: 8.0,
        every: 0.4,
        cursor: Vec2::new(180.0, -40.0),
        orbit: false,
        ui: 1.5,
        crop: CROP,
    };
    while let Some(key) = args.next() {
        let mut value = || args.next().ok_or(format!("{key}: нет значения"));
        match key.as_str() {
            "--secs" => options.secs = value()?.parse()?,
            "--every" => options.every = value()?.parse()?,
            "--ui" => options.ui = value()?.parse()?,
            "--crop" => {
                let raw = value()?;
                let (w, h) = raw.split_once(',').ok_or("--crop w,h")?;
                options.crop = Vec2::new(w.parse()?, h.parse()?);
            }
            "--orbit" => options.orbit = true,
            "--cursor" => {
                let raw = value()?;
                let (x, y) = raw.split_once(',').ok_or("--cursor x,y")?;
                options.cursor = Vec2::new(x.parse()?, y.parse()?);
            }
            other => return Err(format!("неизвестный ключ {other}").into()),
        }
    }
    Ok(options)
}

/// Сцена в кадре `crop` (физические px мира): эффекты, персонаж и курсор поверх фона.
fn shoot(sim: &Sim, crop: Rect, cursor: Vec2) -> (Vec<u32>, usize, usize) {
    let (w, h) = (
        crop.width().round() as usize,
        crop.height().round() as usize,
    );
    let mut ground = vec![0u32; w * h];
    let mut body = vec![0u32; w * h];
    let ui = sim.ui_scale();
    if let Some(mut canvas) = Canvas::new(&mut ground, w, w as u32, h as u32) {
        scene::draw_effects(&mut canvas, crop.min, &sim.effects(), ui);
    }
    if let (Some(CharacterView::Creature { pose, .. }), Some(mut canvas)) = (
        sim.character(),
        Canvas::new(&mut body, w, w as u32, h as u32),
    ) {
        creature::draw(&mut canvas, crop.min, pose);
        if let Some(screen) = sim.character_clip() {
            canvas.clear_outside(Rect::new(screen.min - crop.min, screen.max - crop.min));
        }
    }
    let mut out: Vec<u32> = ground
        .iter()
        .zip(&body)
        .map(|(&under, &over)| over_premultiplied(over, under))
        .collect();
    draw_cursor(&mut out, w, h, cursor - crop.min, ui);
    (out, w, h)
}

fn over_premultiplied(over: u32, under: u32) -> u32 {
    let keep = 255 - (over >> 24);
    let channel = |shift: u32| {
        let (o, u) = (over >> shift & 255, under >> shift & 255);
        (o + u * keep / 255).min(255) << shift
    };
    channel(24) | channel(16) | channel(8) | channel(0)
}

/// Стрелочка курсора: белая с тёмной обводкой.
fn draw_cursor(pixels: &mut [u32], w: usize, h: usize, at: Vec2, ui: f32) {
    let size = 12.0 * ui;
    for y in 0..(size as i32 + 2) {
        for x in 0..=(y / 2 + 1) {
            let (px, py) = (at.x as i32 + x, at.y as i32 + y);
            if px < 0 || py < 0 || px as usize >= w || py as usize >= h {
                continue;
            }
            let edge = x == 0 || x == y / 2 + 1 || y == size as i32 + 1;
            pixels[py as usize * w + px as usize] = if edge { 0xFF10_1010 } else { 0xFFFF_FFFF };
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse()?;
    let screen = Rect::new(Vec2::ZERO, Vec2::new(SCREEN[0], SCREEN[1]) * options.ui);
    let bounds = Bounds::new(vec![screen]).ok_or("экран")?;
    let config = AgentConfig {
        seed: Some(7),
        naughty: true,
        stunts: false,
        cursor_grab: CursorGrabConfig {
            enabled: true,
            ..CursorGrabConfig::default()
        },
        ..AgentConfig::default()
    };
    let dpi = (96.0 * options.ui).round() as u32;
    let mut sim = Sim::new(Character::builtin(options.kind)?, &config, bounds, dpi, 7);
    let start = screen.center() + Vec2::new(0.0, 120.0 * options.ui);
    sim.command(Command::Spawn(Some(start)));
    let mut events = Vec::new();
    let mut cursor = start + options.cursor * options.ui;
    let tick = |sim: &mut Sim, cursor: Vec2, events: &mut Vec<AgentEvent>| {
        let frame = Frame {
            dt: DT,
            cursor: Some(cursor),
            user_input: false,
            permitted: true,
        };
        let moved = sim.step(&frame);
        sim.drain_events(events);
        moved
    };
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut sim, cursor, &mut events);
    }
    events.clear();
    sim.command(Command::Stage(options.stunt));
    let (mut frames, mut size) = (Vec::new(), (0, 0));
    let steps = (options.secs / DT) as usize;
    let every = ((options.every / DT).round() as usize).max(1);
    let half = options.crop * (0.5 * options.ui);
    for step in 0..steps {
        if options.orbit {
            let turn = step as f32 * DT;
            cursor = start + Vec2::from_angle(turn) * (ORBIT_RADIUS * options.ui);
        }
        if let Some(moved) = tick(&mut sim, cursor, &mut events) {
            cursor = moved;
        }
        if step % every == 0 {
            let center = screen.clamp(sim.position() - Vec2::new(0.0, 30.0 * options.ui), 0.0);
            let corner = center - half;
            let corner = Vec2::new(corner.x.floor(), corner.y.floor());
            let crop = Rect::new(corner, corner + half * 2.0);
            let (pixels, w, h) = shoot(&sim, crop, cursor);
            size = (w, h);
            frames.push(pixels);
        }
    }
    for event in &events {
        println!("{event:?}");
    }
    let finished = events.contains(&AgentEvent::StuntFinished(options.stunt));
    println!("{} доиграна: {finished}", options.stunt.name());
    write_sheet(&options.path, &frames, size)?;
    println!("{}", options.path.display());
    Ok(())
}

fn write_sheet(
    path: &PathBuf,
    frames: &[Vec<u32>],
    (w, h): (usize, usize),
) -> Result<(), Box<dyn Error>> {
    let rows = frames.len().div_ceil(COLUMNS).max(1);
    let (width, height) = (COLUMNS * w, rows * h);
    let mut rgb = vec![40u8; width * height * 3];
    for (index, frame) in frames.iter().enumerate() {
        let (left, top) = ((index % COLUMNS) * w, (index / COLUMNS) * h);
        for (i, pixel) in frame.iter().enumerate() {
            let (x, y) = (left + i % w, top + i / w);
            // Тонкая рамка между кадрами
            if i % w == 0 || i / w == 0 {
                continue;
            }
            let keep = 255 - (pixel >> 24);
            let source = [pixel >> 16 & 255, pixel >> 8 & 255, pixel & 255];
            for (channel, (src, back)) in source.into_iter().zip(BACKGROUND).enumerate() {
                rgb[(y * width + x) * 3 + channel] = (src + back * keep / 255).min(255) as u8;
            }
        }
    }
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(path)?),
        width as u32,
        height as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgb)?;
    Ok(())
}
