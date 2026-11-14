//! `rig.json`: разбор и проверки. Размеры — design px, повороты — градусы.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Bone, Driver, Outline, Part, Shape, Skeleton, Socket};
use crate::error::AgentError;
use crate::rig::vec3::{Frame, Vec3};

pub const RIG_FILE: &str = "rig.json";
pub const MAX_RIG_BYTES: usize = 1 << 20;
pub const MAX_BONES: usize = 64;
pub const MAX_PARTS: usize = 256;
pub const MAX_SOCKETS: usize = 64;
pub const MAX_CONTROLS: usize = 32;
/// Костей в одной цепочке, не считая первой.
pub const MAX_CHAIN: usize = 16;
/// Дальше этого от кости ничего не бывает, design px.
const MAX_REACH: f32 = 1024.0;
const MAX_OUTLINE: f32 = 8.0;
/// Обводок у вида и драйверов у кости.
pub const MAX_OUTLINES: usize = 4;
const MAX_DRIVERS: usize = 8;
/// Полуоси диска не короче этого и не вдоль друг друга.
const MIN_AXIS: f32 = 1e-3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RigFile {
    /// Имя цвета → `RRGGBB`.
    palette: BTreeMap<String, String>,
    /// Имя обводки → цвет и ширина; на имя ссылается `rim` части.
    #[serde(default)]
    outlines: BTreeMap<String, OutlineDef>,
    #[serde(default)]
    controls: Vec<String>,
    bones: Vec<BoneDef>,
    #[serde(default)]
    parts: Vec<PartDef>,
    #[serde(default)]
    sockets: Vec<SocketDef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutlineDef {
    color: String,
    width: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoneDef {
    name: String,
    parent: Option<String>,
    #[serde(default)]
    at: [f32; 3],
    /// По земле, подъём носа, вокруг оси вперёд.
    #[serde(default)]
    turn: [f32; 3],
    #[serde(default = "ones")]
    stretch: [f32; 3],
    #[serde(default)]
    ground: bool,
    #[serde(default)]
    drivers: Vec<DriverDef>,
}

/// Значение регулятора, умноженное на эти добавки, двигает кость от покоя.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverDef {
    control: String,
    #[serde(default)]
    turn: [f32; 3],
    #[serde(default)]
    shift: [f32; 3],
    #[serde(default)]
    stretch: [f32; 3],
}

fn ones() -> [f32; 3] {
    [1.0; 3]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartDef {
    bone: String,
    color: String,
    shape: ShapeDef,
    /// Имя обводки из `outlines`.
    rim: Option<String>,
    facing: Option<[f32; 3]>,
    #[serde(default)]
    sink: f32,
    #[serde(default)]
    behind: bool,
    #[serde(default)]
    lift: f32,
    #[serde(default)]
    show: Vec<String>,
    #[serde(default)]
    hide: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum ShapeDef {
    Ball {
        #[serde(default)]
        at: [f32; 3],
        radius: f32,
    },
    Ellipsoid {
        #[serde(default)]
        at: [f32; 3],
        radii: [f32; 3],
    },
    Capsule {
        from: [f32; 3],
        to: [f32; 3],
        radii: [f32; 2],
    },
    Disc {
        #[serde(default)]
        at: [f32; 3],
        axes: [[f32; 3]; 2],
    },
    Chain {
        #[serde(default)]
        from: [f32; 3],
        through: Vec<String>,
        radii: Vec<f32>,
        reach: Option<f32>,
        curve: Option<usize>,
        #[serde(default)]
        bands: Vec<String>,
        #[serde(default)]
        floor: bool,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SocketDef {
    name: String,
    bone: String,
    #[serde(default)]
    at: [f32; 3],
}

impl Skeleton {
    pub fn parse(text: &str) -> Result<Self, AgentError> {
        if text.len() > MAX_RIG_BYTES {
            return Err(fail("файл больше 1 МиБ"));
        }
        let file: RigFile = serde_json::from_str(text).map_err(fail)?;
        build(file).map_err(fail)
    }
}

fn fail(reason: impl ToString) -> AgentError {
    AgentError::Asset {
        file: RIG_FILE.to_owned(),
        reason: reason.to_string(),
    }
}

/// Имена костей, цветов и регуляторов файла → номера.
struct Names<'a> {
    bones: &'a [Bone],
    palette: &'a BTreeMap<String, String>,
    controls: &'a [String],
    rims: &'a [&'a str],
}

impl Names<'_> {
    fn bone(&self, name: &str) -> Result<usize, String> {
        let found = self.bones.iter().position(|bone| bone.name == name);
        found.ok_or_else(|| format!("нет кости «{name}»"))
    }

    fn color(&self, name: &str) -> Result<u32, String> {
        let hex = self.palette.get(name);
        let hex = hex.ok_or_else(|| format!("нет цвета «{name}» в palette"))?;
        parse_color(hex).ok_or_else(|| format!("цвет «{name}»: нужен RRGGBB"))
    }

    fn controls(&self, names: &[String]) -> Result<Vec<usize>, String> {
        let index = |name: &String| {
            let found = self.controls.iter().position(|control| control == name);
            found.ok_or_else(|| format!("нет регулятора «{name}» в controls"))
        };
        names.iter().map(index).collect()
    }
}

fn build(file: RigFile) -> Result<Skeleton, String> {
    if file.bones.is_empty() || file.bones.len() > MAX_BONES {
        return Err(format!("костей должно быть от 1 до {MAX_BONES}"));
    }
    if file.parts.len() > MAX_PARTS {
        return Err(format!("частей больше {MAX_PARTS}"));
    }
    if file.sockets.len() > MAX_SOCKETS {
        return Err(format!("сокетов больше {MAX_SOCKETS}"));
    }
    if file.controls.len() > MAX_CONTROLS {
        return Err(format!("регуляторов больше {MAX_CONTROLS}"));
    }
    for (index, name) in file.controls.iter().enumerate() {
        check_name(
            "регулятора",
            name,
            file.controls[..index].iter().map(String::as_str),
        )?;
    }
    if file.outlines.len() > MAX_OUTLINES {
        return Err(format!("обводок больше {MAX_OUTLINES}"));
    }
    let mut bones: Vec<Bone> = Vec::with_capacity(file.bones.len());
    for def in file.bones {
        let bone = bone(def, &bones, &file.controls)?;
        bones.push(bone);
    }
    let rims: Vec<&str> = file.outlines.keys().map(String::as_str).collect();
    let names = Names {
        bones: &bones,
        palette: &file.palette,
        controls: &file.controls,
        rims: &rims,
    };
    let mut parts = Vec::with_capacity(file.parts.len());
    for (number, def) in file.parts.into_iter().enumerate() {
        let part = part(def, &names).map_err(|reason| format!("часть {number}: {reason}"))?;
        parts.push(part);
    }
    let mut sockets: Vec<Socket> = Vec::with_capacity(file.sockets.len());
    for def in file.sockets {
        check_name("сокета", &def.name, sockets.iter().map(|s| s.name.as_str()))?;
        let at = point(def.at).map_err(|reason| format!("сокет «{}»: {reason}", def.name))?;
        let bone = names.bone(&def.bone)?;
        sockets.push(Socket {
            name: def.name,
            bone,
            at,
        });
    }
    let outline = |(name, def): (&String, &OutlineDef)| {
        if !(def.width > 0.0 && def.width <= MAX_OUTLINE) {
            return Err(format!(
                "обводка «{name}»: width должен быть в (0, {MAX_OUTLINE}]"
            ));
        }
        Ok(Outline {
            color: names.color(&def.color)?,
            width: def.width,
        })
    };
    let outlines = file
        .outlines
        .iter()
        .map(outline)
        .collect::<Result<_, _>>()?;
    Ok(Skeleton {
        bones,
        parts,
        sockets,
        controls: file.controls,
        outlines,
    })
}

/// Родитель обязан стоять раньше: порядок файла — порядок обхода, циклов быть не может.
fn bone(def: BoneDef, before: &[Bone], controls: &[String]) -> Result<Bone, String> {
    check_name("кости", &def.name, before.iter().map(|b| b.name.as_str()))?;
    let wrap = |reason: String| format!("кость «{}»: {reason}", def.name);
    let parent = def
        .parent
        .as_deref()
        .map(|parent| {
            before
                .iter()
                .position(|bone| bone.name == parent)
                .ok_or_else(|| format!("родитель «{parent}» должен стоять раньше"))
        })
        .transpose()
        .map_err(wrap)?;
    if !def.turn.iter().all(|angle| angle.is_finite()) {
        return Err(wrap("turn должен быть конечным".to_owned()));
    }
    if def.drivers.len() > MAX_DRIVERS {
        return Err(wrap(format!("драйверов больше {MAX_DRIVERS}")));
    }
    let driver = |driver: &DriverDef| {
        let found = controls.iter().position(|name| *name == driver.control);
        let control =
            found.ok_or_else(|| format!("нет регулятора «{}» в controls", driver.control))?;
        let numbers = driver.turn.iter().chain(&driver.stretch);
        if !numbers.into_iter().all(|v| v.abs() <= MAX_REACH) {
            return Err(format!(
                "драйвер «{}»: числа в ±{MAX_REACH}",
                driver.control
            ));
        }
        Ok(Driver {
            control,
            turn: driver.turn.map(f32::to_radians),
            shift: point(driver.shift)?,
            stretch: driver.stretch,
        })
    };
    let drivers = def.drivers.iter().map(driver).collect::<Result<_, _>>();
    if !def.stretch.iter().all(|v| (0.0..=MAX_REACH).contains(v)) {
        return Err(wrap(format!("stretch должен быть в [0, {MAX_REACH}]")));
    }
    let [yaw, pitch, roll] = def.turn.map(f32::to_radians);
    Ok(Bone {
        at: point(def.at).map_err(wrap)?,
        rest: Frame::new(yaw, pitch, roll),
        stretch: def.stretch,
        ground: def.ground,
        drivers: drivers.map_err(wrap)?,
        parent,
        name: def.name,
    })
}

fn part(def: PartDef, names: &Names) -> Result<Part, String> {
    if !(0.0..=1.0).contains(&def.sink) {
        return Err("sink должен быть в [0, 1]".to_owned());
    }
    if def.sink > 0.0 && def.facing.is_none() {
        return Err("sink бывает только у части с facing".to_owned());
    }
    let bone = names.bone(&def.bone)?;
    let shape = shape(def.shape, bone, names)?;
    if def.facing.is_some() && matches!(shape, Shape::Chain { .. }) {
        return Err("цепочка не бывает лицом к зрителю".to_owned());
    }
    if def.lift.is_nan() || def.lift.abs() > MAX_REACH {
        return Err(format!("lift должен быть в ±{MAX_REACH}"));
    }
    let rim = |name: &String| {
        let found = names.rims.iter().position(|rim| rim == name);
        found.ok_or_else(|| format!("нет обводки «{name}» в outlines"))
    };
    Ok(Part {
        bone,
        color: names.color(&def.color)?,
        shape,
        rim: def.rim.as_ref().map(rim).transpose()?,
        facing: def.facing.map(point).transpose()?,
        sink: def.sink,
        behind: def.behind,
        lift: def.lift,
        show: names.controls(&def.show)?,
        hide: names.controls(&def.hide)?,
    })
}

fn check_name<'a>(
    what: &str,
    name: &str,
    mut taken: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!("имя {what} пустое"));
    }
    if taken.any(|other| other == name) {
        return Err(format!("имя {what} «{name}» повторяется"));
    }
    Ok(())
}

fn radii<R: AsRef<[f32]>>(radii: R, what: &str) -> Result<R, String> {
    let list = radii.as_ref();
    let ok = list.iter().all(|r| (0.0..=MAX_REACH).contains(r));
    if ok && list.iter().any(|r| *r > 0.0) {
        Ok(radii)
    } else {
        Err(format!(
            "радиусы {what} должны быть в [0, {MAX_REACH}] и не все нули"
        ))
    }
}

fn shape(def: ShapeDef, bone: usize, names: &Names) -> Result<Shape, String> {
    Ok(match def {
        ShapeDef::Ball { at, radius } => Shape::Ball {
            at: point(at)?,
            radius: size(radius)?,
        },
        ShapeDef::Ellipsoid { at, radii } => Shape::Ellipsoid {
            at: point(at)?,
            radii: [size(radii[0])?, size(radii[1])?, size(radii[2])?],
        },
        ShapeDef::Capsule { from, to, radii: r } => Shape::Capsule {
            from: point(from)?,
            to: point(to)?,
            radii: radii(r, "капсулы")?,
        },
        ShapeDef::Disc { at, axes } => {
            let axes = [point(axes[0])?, point(axes[1])?];
            let least = axes[0].length().min(axes[1].length());
            let flat = axes[0].cross(axes[1]).length();
            if least < MIN_AXIS || flat < MIN_AXIS * least {
                return Err(
                    "полуоси диска не должны быть нулевыми или лежать вдоль друг друга".to_owned(),
                );
            }
            Shape::Disc {
                at: point(at)?,
                axes,
            }
        }
        ShapeDef::Chain {
            from,
            through,
            radii: r,
            reach,
            curve,
            bands,
            floor,
        } => {
            if through.is_empty() || through.len() > MAX_CHAIN {
                return Err(format!("в цепочке должно быть от 1 до {MAX_CHAIN} костей"));
            }
            if !(2..=MAX_CHAIN).contains(&r.len()) || bands.len() > MAX_CHAIN {
                return Err(format!(
                    "радиусов цепочки от 2 до {MAX_CHAIN}, цветов не больше"
                ));
            }
            // Дуга — одна опорная точка между началом и концом
            if curve.is_some_and(|links| through.len() != 2 || !(2..=MAX_CHAIN).contains(&links)) {
                return Err(format!(
                    "curve — от 2 до {MAX_CHAIN} звеньев у цепочки через две кости"
                ));
            }
            let through = through
                .iter()
                .map(|name| names.bone(name))
                .collect::<Result<Vec<_>, _>>()?;
            if through.contains(&bone) {
                return Err("цепочка не проходит через свою кость".to_owned());
            }
            Shape::Chain {
                from: point(from)?,
                through,
                radii: radii(r, "цепочки")?,
                reach: reach.map(size).transpose()?,
                curve,
                bands: bands
                    .iter()
                    .map(|name| names.color(name))
                    .collect::<Result<_, _>>()?,
                floor,
            }
        }
    })
}

fn point(at: [f32; 3]) -> Result<Vec3, String> {
    if at.iter().all(|v| v.abs() <= MAX_REACH) {
        Ok(Vec3::new(at[0], at[1], at[2]))
    } else {
        Err(format!("координаты должны быть в ±{MAX_REACH}"))
    }
}

fn size(value: f32) -> Result<f32, String> {
    if value > 0.0 && value <= MAX_REACH {
        Ok(value)
    } else {
        Err(format!("размер должен быть в (0, {MAX_REACH}]"))
    }
}

fn parse_color(hex: &str) -> Option<u32> {
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}
