//! `agent.json`: разбор и проверки, не требующие самих листов.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    config::{BehaviorConfig, MotionConfig},
    error::AgentError,
    rig::RigKind,
};

pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
pub const MAX_ANIMATIONS: usize = 256;
pub const MAX_ACTIONS: usize = 4096;
pub const MAX_FRAMES: u32 = 256;
const MAX_SCALE: f32 = 16.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    /// Тело, которое рисует движок; без него набор спрайтовый.
    pub rig: Option<String>,
    /// `rig`, прошедший проверку.
    #[serde(skip)]
    pub kind: Option<RigKind>,
    /// У тела не используется, но обязателен схемой.
    pub frame_size: [u32; 2],
    pub anchor: [f32; 2],
    #[serde(default = "one")]
    pub scale: f32,
    pub animations: BTreeMap<String, Animation>,
    pub actions: BTreeMap<String, Action>,
    #[serde(default)]
    pub motion: MotionConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    /// Обязателен у спрайтового набора, запрещён у тела.
    pub file: Option<String>,
    pub frames: Frames,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    pub anchor: Option<[f32; 2]>,
    #[serde(default)]
    pub events: Vec<Event>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Frames {
    Grid(Grid),
    List(Vec<FrameRect>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grid {
    pub count: u32,
    pub duration_ms: u32,
    pub columns: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub duration_ms: u32,
    #[serde(default)]
    pub offset: [f32; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub frame: u32,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub animation: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(default = "yes")]
    pub interruptible: bool,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

pub fn parse(text: &str) -> Result<Manifest, AgentError> {
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(fail("файл больше 1 МиБ"));
    }
    let mut manifest: Manifest = serde_json::from_str(text).map_err(fail)?;
    if manifest.frame_size.contains(&0) {
        return Err(fail("frame_size не может быть нулевым"));
    }
    if !(manifest.scale > 0.0 && manifest.scale <= MAX_SCALE) {
        return Err(fail("scale должен быть в (0, 16]"));
    }
    if manifest.animations.len() > MAX_ANIMATIONS {
        return Err(fail("анимаций больше 256"));
    }
    if manifest.actions.len() > MAX_ACTIONS {
        return Err(fail("действий больше 4096"));
    }
    check_point("anchor", manifest.anchor)?;
    manifest.kind = manifest.rig.as_deref().map(rig_kind).transpose()?;
    for (name, animation) in &manifest.animations {
        check_animation(manifest.kind, animation)
            .map_err(|reason| fail(format!("анимация «{name}»: {reason}")))?;
        if let Some(anchor) = animation.anchor {
            check_point("anchor", anchor)?;
        }
    }
    Ok(manifest)
}

fn rig_kind(name: &str) -> Result<RigKind, AgentError> {
    RigKind::from_name(name).ok_or_else(|| {
        fail(format!(
            "неизвестное тело «{name}», есть: goose, pig, dachshund"
            "неизвестное тело «{name}», есть: goose, pig, elephant"
        ))
        let known = RigKind::ALL.map(RigKind::name).join(", ");
        fail(format!("неизвестное тело «{name}», есть: {known}"))
    })
}

/// Спрайтовой анимации нужен лист, анимации тела — только время.
fn check_animation(kind: Option<RigKind>, animation: &Animation) -> Result<(), &'static str> {
    match (&animation.file, kind) {
        (Some(file), None) => check_path(file),
        (None, None) => Err("у спрайтового набора обязателен file"),
        (Some(_), Some(_)) => Err("у набора с телом file не нужен: листов нет"),
        (None, Some(_)) if matches!(animation.frames, Frames::Grid(_)) => Ok(()),
        (None, Some(_)) => Err("у набора с телом кадры задаются сеткой {count, duration_ms}"),
    }
}

pub fn check_point(what: &str, point: [f32; 2]) -> Result<(), AgentError> {
    if point.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(fail(format!("{what} должен быть конечным числом")))
    }
}

/// Относительный путь с `/` без `..`, диска и ведущего `/`.
fn check_path(file: &str) -> Result<(), &'static str> {
    let bad = file.is_empty()
        || file.starts_with('/')
        || file.contains(['\\', ':'])
        || file.split('/').any(|part| matches!(part, "" | "." | ".."));
    if bad {
        Err("file должен быть относительным путём с «/» без «..»")
    } else {
        Ok(())
    }
}

pub fn fail(reason: impl ToString) -> AgentError {
    AgentError::Manifest(reason.to_string())
}
