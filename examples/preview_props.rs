//! Контактный лист предметов и частиц: виды на земле и в воздухе, стадии, все `Burst` по времени.
//! `cargo run -p ui-agent --example preview_props -- <out.png> [ui] [bursts]`
//! Без третьего аргумента — предметы, с `bursts` — частицы.

use std::{error::Error, fs::File, io::BufWriter, path::PathBuf};

use ui_agent::{
    effects::{
        footprints::Footprints,
        leaves::Leaves,
        mud::Mud,
        particles::{Burst, Particles},
        props::{PropKind, Props},
    },
    math::Vec2,
    render::{
        Canvas,
        scene::{Effects, draw_effects},
    },
    rng::Rng,
};

const COLUMNS: usize = 9;
const BACKGROUND: [u32; 3] = [118, 124, 134];
const DT: f32 = 1.0 / 60.0;
/// Моменты жизни частиц, с.
const MOMENTS: [f32; 3] = [0.1, 0.4, 0.8];
/// Высота предмета «в воздухе», логические px.
const AIR: f32 = 16.0;

const KINDS: [PropKind; 14] = [
    PropKind::Ball,
    PropKind::Apple,
    PropKind::Acorn,
    PropKind::Peel,
    PropKind::Pile,
    PropKind::Egg,
    PropKind::Coin,
    PropKind::Bucket,
    PropKind::Crumb,
    PropKind::Pebble,
    PropKind::Feather,
    PropKind::Arrow,
    PropKind::Butterfly,
    PropKind::Slipper,
    PropKind::Paper,
];

/// Одна ячейка листа: предмет или всплеск на заданный момент.
enum Cell {
    Prop {
        kind: PropKind,
        stage: u8,
        z: f32,
        angle: f32,
        age: f32,
    },
    Burst(Burst, f32),
}

fn cells() -> Vec<Cell> {
    let prop = |kind, stage, z, angle| Cell::Prop {
        kind,
        stage,
        z,
        angle,
        age: 0.3,
    };
    let mut cells = Vec::new();
    for z in [0.0, AIR] {
        cells.extend(KINDS.map(|kind| prop(kind, 0, z, 0.5)));
    }
    for stage in 1..=3 {
        cells.push(prop(PropKind::Apple, stage, 0.0, 0.0));
        cells.push(prop(PropKind::Egg, stage, 0.0, 0.0));
    }
    cells.push(prop(PropKind::Pile, 1, 0.0, 0.0));
    cells.push(Cell::Prop {
        kind: PropKind::Pile,
        stage: 0,
        z: 0.0,
        angle: 0.0,
        age: 0.9,
    });
    for angle in [0.5, 1.2, 1.5, 2.2] {
        cells.push(prop(PropKind::Coin, 0, 0.0, angle));
    }
    for burst in Burst::ALL {
        cells.extend(MOMENTS.map(|at| Cell::Burst(burst, at)));
    }
    cells
}

fn render(cells: &[Cell], ui: f32, size: Vec2) -> Result<(Vec<u8>, usize, usize), Box<dyn Error>> {
    let (cw, ch) = ((size.x * ui) as usize, (size.y * ui) as usize);
    let rows = cells.len().div_ceil(COLUMNS);
    let (width, height) = (COLUMNS * cw, rows * ch);
    let mut rgb = vec![0u8; width * height * 3];
    let ground = Vec2::new(cw as f32 / 2.0, ch as f32 * 0.68);
    for (index, cell) in cells.iter().enumerate() {
        let (mut props, mut particles) = (Props::new(4), Particles::new(64));
        match *cell {
            Cell::Prop {
                kind,
                stage,
                z,
                angle,
                age,
            } => {
                let id = props.spawn(kind, ground, z * ui, ui, 60.0);
                let prop = props.get_mut(id).ok_or("предмет")?;
                (prop.stage, prop.angle, prop.age) = (stage, angle, age);
            }
            Cell::Burst(burst, at) => {
                let mut rng = Rng::new(11);
                particles.burst(burst, ground, ui, &mut rng);
                for _ in 0..(at / DT) as usize {
                    particles.update(DT);
                }
            }
        }
        let (prints, leaves, mud) = (Footprints::new(1, 1.0, 1.0), Leaves::new(1), Mud::new(1));
        let effects = Effects {
            footprints: &prints,
            print_radius: 4.5,
            particles: &particles,
            leaves: &leaves,
            mud: &mud,
            mud_prints: &prints,
            props: &props,
            scribble: &prints,
            flock: &[],
        };
        let mut pixels = vec![0u32; cw * ch];
        let mut canvas = Canvas::new(&mut pixels, cw, cw as u32, ch as u32).ok_or("холст")?;
        draw_effects(&mut canvas, Vec2::ZERO, &effects, ui);
        let (left, top) = ((index % COLUMNS) * cw, (index / COLUMNS) * ch);
        for (i, pixel) in pixels.iter().enumerate() {
            let (x, y) = (left + i % cw, top + i / cw);
            let alpha = pixel >> 24;
            let source = [pixel >> 16 & 255, pixel >> 8 & 255, pixel & 255];
            for (channel, (src, back)) in source.into_iter().zip(BACKGROUND).enumerate() {
                rgb[(y * width + x) * 3 + channel] = (src + back * (255 - alpha) / 255) as u8;
            }
        }
    }
    Ok((rgb, width, height))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path: PathBuf = args.next().ok_or("первый аргумент — путь к PNG")?.into();
    let ui: f32 = args.next().map_or(Ok(2.0), |text| text.parse())?;
    let bursts = args.next().is_some_and(|part| part == "bursts");
    let cells = cells()
        .into_iter()
        .filter(|cell| matches!(cell, Cell::Burst(..)) == bursts)
        .collect::<Vec<_>>();
    // Предметам хватает малой ячейки, частицы разлетаются шире
    let size = if bursts {
        Vec2::new(100.0, 90.0)
    } else {
        Vec2::new(44.0, 50.0)
    };
    let (rgb, width, height) = render(&cells, ui, size)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(&path)?),
        width as u32,
        height as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgb)?;
    println!("{}", path.display());
    Ok(())
}
