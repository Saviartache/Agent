//! Контактный лист тела для проверки глазами: ходьба по моментам и жесты × стороны.
//! `cargo run -p ui-agent --example preview_rig -- <goose|pig|dachshund> <out.png> [cycle,<жест>,…]`
//! `cargo run -p ui-agent --example preview_rig -- <goose|pig|elephant> <out.png> [cycle,<жест>,…]`
//! `cycle` — кадры ходьбы и бега; `mud` — тело в грязи после лужи;
//! жест — имя встроенного действия (`react`, `sleep`, …);
//! `antic:поле=число;eyes=crossed;tongue=1;mud=1` — накладка сценки и вид тела по всем сторонам.
//! Без фильтра рисуется всё.

use std::{error::Error, fs::File, io::BufWriter, path::PathBuf};

use ui_agent::{
    math::Vec2,
    render::{Canvas, creature},
    rig::{Antic, Creature, Eyes, Footfall, Gesture, Look, Pose, RigInput, RigKind},
    rng::Rng,
};

/// Масштаб превью: 2× от логических пикселей.
const SCALE: f32 = 2.0;
const CELL: [usize; 2] = [200, 210];
/// Точка на земле в ячейке.
const GROUND: Vec2 = Vec2::new(100.0, 165.0);
const COLUMNS: usize = 8;
const BACKGROUND: [u32; 3] = [118, 124, 134];
const DT: f32 = 1.0 / 60.0;
const SETTLE_SECS: f32 = 1.6;
const WALK_SPEED: f32 = 70.0;
const RUN_SPEED: f32 = 230.0;
/// Шаг между кадрами цикла ходьбы, с.
const CYCLE_SAMPLE_SECS: f32 = 0.09;
const CYCLE_WARM_UP_SECS: f32 = 1.0;
/// Доли клипа, на которых снимается жест.
const PHASES: [f32; 2] = [0.3, 0.7];

/// По кругу от «вправо» через «вниз»: повороты видны вместе с прямыми ракурсами.
const HEADINGS: [Vec2; 8] = [
    Vec2::new(1.0, 0.0),
    Vec2::new(0.7, 0.7),
    Vec2::new(0.0, 1.0),
    Vec2::new(-0.7, 0.7),
    Vec2::new(-1.0, 0.0),
    Vec2::new(-0.7, -0.7),
    Vec2::new(0.0, -1.0),
    Vec2::new(0.7, -0.7),
];

const GESTURES: [(&str, Gesture); 17] = [
    ("idle", Gesture::Idle),
    ("walk", Gesture::Walk),
    ("run", Gesture::Run),
    ("look", Gesture::Look),
    ("react", Gesture::React),
    ("interact", Gesture::Interact),
    ("celebrate", Gesture::Celebrate),
    ("think", Gesture::Think),
    ("busy", Gesture::Busy),
    ("warning", Gesture::Warning),
    ("error", Gesture::Error),
    ("sleep", Gesture::Sleep),
    ("wake_up", Gesture::WakeUp),
    ("dragged", Gesture::Dragged),
    ("spawn", Gesture::Spawn),
    ("despawn", Gesture::Despawn),
    ("wave", Gesture::Wave),
];

/// Тело идёт по миру вдоль `heading`; `frame` отдаёт позу после каждого кадра.
struct Run {
    rig: Creature,
    rng: Rng,
    input: RigInput,
    steps: Vec<Footfall>,
}

impl Run {
    fn new(kind: RigKind, gesture: Gesture, progress: f32, heading: Vec2, speed: f32) -> Self {
        Self {
            rig: Creature::new(kind, GROUND, heading, SCALE),
            rng: Rng::new(7),
            input: RigInput {
                pos: GROUND,
                vel: heading * (speed * SCALE),
                heading,
                look_at: None,
                gesture,
                progress,
                scale: SCALE,
                grounded: gesture != Gesture::Dragged,
                antic: Antic::default(),
            },
            steps: Vec::new(),
        }
    }

    fn frame(&mut self) -> &Pose {
        self.input.pos += self.input.vel * DT;
        self.rig
            .update(DT, &self.input, &mut self.rng, &mut self.steps);
        self.steps.clear();
        self.rig.pose()
    }
}

/// Тело после `SETTLE_SECS` секунд в заданном жесте.
fn settled(kind: RigKind, gesture: Gesture, progress: f32, heading: Vec2, speed: f32) -> Pose {
    let mut run = Run::new(kind, gesture, progress, heading, speed);
    for _ in 1..(SETTLE_SECS / DT) as usize {
        run.frame();
    }
    run.frame().clone()
}

/// Кадры цикла ходьбы с равным шагом по времени.
fn cycle(kind: RigKind, heading: Vec2, speed: f32) -> Vec<Pose> {
    let mut run = Run::new(kind, Gesture::Walk, 0.0, heading, speed);
    for _ in 0..(CYCLE_WARM_UP_SECS / DT) as usize {
        run.frame();
    }
    let every = (CYCLE_SAMPLE_SECS / DT).round() as usize;
    (0..COLUMNS)
        .map(|_| {
            for _ in 1..every {
                run.frame();
            }
            run.frame().clone()
        })
        .collect()
}

fn gesture_speed(gesture: Gesture) -> f32 {
    match gesture {
        Gesture::Walk => WALK_SPEED,
        Gesture::Run => RUN_SPEED,
        _ => 0.0,
    }
}

/// `antic:roll=3.14;eyes=crossed;mud=1` — накладка сценки и вид тела.
fn parse_antic(spec: &str) -> Result<(Antic, Look), String> {
    let (mut antic, mut look) = (Antic::default(), Look::default());
    for pair in spec.split(';').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .ok_or(format!("{pair}: нужно ключ=значение"))?;
        if key == "eyes" {
            antic.eyes = match value {
                "normal" => Eyes::Normal,
                "crossed" => Eyes::Crossed,
                "wide" => Eyes::Wide,
                _ => return Err(format!("глаза: normal, crossed, wide, а не {value}")),
            };
            continue;
        }
        if key == "tongue" {
            antic.tongue = value == "1" || value == "true";
            continue;
        }
        let v: f32 = value.parse().map_err(|_| format!("{pair}: не число"))?;
        let field = match key {
            "lift" => &mut antic.lift,
            "squash_x" => &mut antic.squash.x,
            "squash_y" => &mut antic.squash.y,
            "roll" => &mut antic.roll,
            "pitch" => &mut antic.pitch,
            "spin" => &mut antic.spin,
            "neck_forward" => &mut antic.neck_forward,
            "neck_up" => &mut antic.neck_up,
            "neck_side" => &mut antic.neck_side,
            "head_pitch" => &mut antic.head_pitch,
            "head_roll" => &mut antic.head_roll,
            "head_yaw" => &mut antic.head_yaw,
            "crouch" => &mut antic.crouch,
            "wobble" => &mut antic.wobble,
            "lids" => &mut antic.lids,
            "tuck" => &mut antic.tuck,
            "mouth" => &mut antic.mouth,
            "wings" => &mut antic.wings,
            "flap" => &mut antic.flap,
            "knot" => &mut antic.knot,
            "wag" => &mut antic.wag,
            "mud" => &mut look.mud,
            "bloat" => &mut look.bloat,
            "snout" => &mut look.snout,
            _ => return Err(format!("нет такого поля: {key}")),
        };
        *field = v;
    }
    Ok((antic, look))
}

/// Тело в накладке по всем сторонам, стоя.
fn posed(kind: RigKind, antic: Antic, look: Look) -> Vec<Pose> {
    HEADINGS
        .into_iter()
        .map(|heading| {
            let mut run = Run::new(kind, Gesture::Idle, 0.5, heading, 0.0);
            run.input.antic = antic;
            run.rig.set_look(look);
            for _ in 1..(SETTLE_SECS / DT) as usize {
                run.frame();
            }
            run.frame().clone()
        })
        .collect()
}

fn build(kind: RigKind, filter: Option<&str>) -> Result<Vec<Pose>, String> {
    let wanted = |name: &str| filter.is_none_or(|list| list.split(',').any(|item| item == name));
    let mut cells = Vec::new();
    for spec in filter
        .into_iter()
        .flat_map(|list| list.split(','))
        .filter_map(|item| item.strip_prefix("antic:"))
    {
        let (antic, look) = parse_antic(spec)?;
        cells.extend(posed(kind, antic, look));
    }
    if wanted("cycle") {
        for speed in [WALK_SPEED, RUN_SPEED] {
            for heading in HEADINGS {
                cells.extend(cycle(kind, heading, speed));
            }
        }
    }
    if wanted("mud") {
        // Вымазанное тело: стоя и на ходу
        for speed in [0.0, WALK_SPEED] {
            for heading in HEADINGS {
                let gesture = if speed > 0.0 {
                    Gesture::Walk
                } else {
                    Gesture::Idle
                };
                let mut pose = settled(kind, gesture, 0.5, heading, speed);
                pose.look.mud = 1.0;
                cells.push(pose);
            }
        }
    }
    let shown = GESTURES.into_iter().filter(|(name, _)| wanted(name));
    for (_, gesture) in shown {
        for heading in HEADINGS {
            for progress in PHASES {
                let speed = gesture_speed(gesture);
                cells.push(settled(kind, gesture, progress, heading, speed));
            }
        }
    }
    if cells.is_empty() {
        return Err(format!("нет таких жестов: {}", filter.unwrap_or_default()));
    }
    Ok(cells)
}

/// RGB-картинка листа и её размеры.
fn render(cells: &[Pose]) -> Result<(Vec<u8>, usize, usize), Box<dyn Error>> {
    let rows = cells.len().div_ceil(COLUMNS);
    let (width, height) = (COLUMNS * CELL[0], rows * CELL[1]);
    let mut rgb = vec![0u8; width * height * 3];
    for (index, pose) in cells.iter().enumerate() {
        let (left, top) = ((index % COLUMNS) * CELL[0], (index / COLUMNS) * CELL[1]);
        let mut pixels = vec![0u32; CELL[0] * CELL[1]];
        let mut canvas = Canvas::new(&mut pixels, CELL[0], CELL[0] as u32, CELL[1] as u32)
            .ok_or("холст ячейки")?;
        // Холст следует за телом: оно всегда в точке `GROUND` ячейки
        creature::draw(&mut canvas, pose.pos - GROUND, pose);
        for (i, pixel) in pixels.iter().enumerate() {
            let (x, y) = (left + i % CELL[0], top + i / CELL[0]);
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
    let kind = args
        .next()
        .and_then(|name| RigKind::from_name(&name))
        .ok_or("первый аргумент — тело: goose, pig или dachshund")?;
        .ok_or("первый аргумент — тело: goose, pig или elephant")?;
    let path: PathBuf = args.next().ok_or("второй аргумент — путь к PNG")?.into();
    let cells = build(kind, args.next().as_deref())?;
    let (rgb, width, height) = render(&cells)?;
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
