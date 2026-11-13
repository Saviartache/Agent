//! Загрузка набора персонажа: манифест + листы PNG → готовые к рисованию таблицы.

use std::{
    collections::HashMap,
    fs::File,
    io::{self, Cursor, Read},
    path::Path,
};

use png::{ColorType, Transformations};

use super::manifest::{self, Animation, Frames, MAX_FRAMES, MAX_MANIFEST_BYTES, Manifest, fail};
use crate::{
    action::{ActionId, ActionSet, ClipId},
    animation::clip::{Clip, Frame, Marker, SheetId},
    config::{BehaviorConfig, MotionConfig},
    error::AgentError,
    math::Vec2,
    rig::RigKind,
};

const MAX_PNG_BYTES: usize = 16 << 20;
const MAX_SIDE: u32 = 4096;
// 64 МиБ пикселей по 4 байта.
const MAX_TOTAL_PIXELS: usize = 16 << 20;

type ReadFile<'a> = &'a dyn Fn(&str) -> io::Result<Vec<u8>>;

/// Пиксели с premultiplied alpha, `0xAARRGGBB`.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

pub struct ActionEntry {
    pub name: String,
    pub clip: ClipId,
    pub priority: u8,
    pub interruptible: bool,
}

pub struct Character {
    id: String,
    rig: Option<RigKind>,
    scale: f32,
    frame_size: [u32; 2],
    sheets: Vec<Sheet>,
    clips: Vec<Clip>,
    actions: Vec<ActionEntry>,
    set: ActionSet,
    motion: MotionConfig,
    behavior: BehaviorConfig,
}

impl Character {
    pub fn load_dir(dir: &Path) -> Result<Self, AgentError> {
        let text = read_limited(&dir.join("agent.json"), MAX_MANIFEST_BYTES)?;
        let text = String::from_utf8(text).map_err(|_| fail("agent.json не в UTF-8"))?;
        Self::from_source(&text, &|file| read_limited(&dir.join(file), MAX_PNG_BYTES))
    }

    /// Набор, вшитый в бинарник: процедурное тело, листов у него нет.
    pub fn builtin(kind: RigKind) -> Result<Self, AgentError> {
        let manifest = match kind {
            RigKind::Goose => include_str!("../../assets/goose/agent.json"),
            RigKind::Pig => include_str!("../../assets/pig/agent.json"),
            RigKind::Dachshund => include_str!("../../assets/dachshund/agent.json"),
        };
        Self::from_source(manifest, &|_| Err(io::ErrorKind::NotFound.into()))
    }

    fn from_source(text: &str, read: ReadFile) -> Result<Self, AgentError> {
        let manifest = manifest::parse(text)?;
        let mut sheets = Sheets::default();
        let mut clips = Vec::with_capacity(manifest.animations.len());
        let mut clip_ids = HashMap::new();
        for (name, animation) in &manifest.animations {
            let sheet = match &animation.file {
                Some(file) => {
                    let id = sheets.load(file, read)?;
                    Some((id, sheets.size(id)))
                }
                None => None,
            };
            clip_ids.insert(name.as_str(), ClipId(clips.len() as u16));
            clips.push(build_clip(&manifest, name, animation, sheet)?);
        }

        let mut actions = Vec::with_capacity(manifest.actions.len());
        for (name, action) in &manifest.actions {
            let clip = *clip_ids.get(action.animation.as_str()).ok_or_else(|| {
                fail(format!(
                    "действие «{name}»: нет анимации «{}»",
                    action.animation
                ))
            })?;
            actions.push(ActionEntry {
                name: name.clone(),
                clip,
                priority: action.priority,
                interruptible: action.interruptible,
            });
        }
        let set = ActionSet::resolve(|name| find_action(&actions, name))
            .ok_or_else(|| fail("обязательны действия idle и walk"))?;

        Ok(Self {
            id: manifest.id,
            rig: manifest.kind,
            scale: manifest.scale,
            frame_size: manifest.frame_size,
            sheets: sheets.list,
            clips,
            actions,
            set,
            motion: manifest.motion,
            behavior: manifest.behavior,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Тело, которое рисует движок; `None` — спрайтовый набор.
    pub fn rig(&self) -> Option<RigKind> {
        self.rig
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn frame_size(&self) -> [u32; 2] {
        self.frame_size
    }

    pub fn actions(&self) -> &ActionSet {
        &self.set
    }

    /// Имена всех действий набора, встроенных и своих.
    pub fn action_names(&self) -> impl Iterator<Item = &str> {
        self.actions.iter().map(|action| action.name.as_str())
    }

    pub fn action_id(&self, name: &str) -> Option<ActionId> {
        find_action(&self.actions, name)
    }

    pub fn action(&self, id: ActionId) -> &ActionEntry {
        &self.actions[usize::from(id.0)]
    }

    pub fn clip(&self, id: ClipId) -> &Clip {
        &self.clips[usize::from(id.0)]
    }

    pub fn sheet(&self, id: SheetId) -> &Sheet {
        &self.sheets[usize::from(id.0)]
    }

    pub fn motion(&self) -> &MotionConfig {
        &self.motion
    }

    pub fn behavior(&self) -> &BehaviorConfig {
        &self.behavior
    }
}

fn find_action(actions: &[ActionEntry], name: &str) -> Option<ActionId> {
    actions
        .iter()
        .position(|action| action.name == name)
        .map(|index| ActionId(index as u16))
}

/// Читает не больше `limit + 1` байт: превышение видно по длине.
fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[derive(Default)]
struct Sheets {
    list: Vec<Sheet>,
    ids: HashMap<String, SheetId>,
    pixels: usize,
}

impl Sheets {
    /// Каждый файл декодируется один раз.
    fn load(&mut self, file: &str, read: ReadFile) -> Result<SheetId, AgentError> {
        if let Some(id) = self.ids.get(file) {
            return Ok(*id);
        }
        let bytes = read(file).map_err(|e| asset(file, format!("не прочитан: {e}")))?;
        if bytes.len() > MAX_PNG_BYTES {
            return Err(asset(file, "файл больше 16 МиБ"));
        }
        let sheet = decode(file, &bytes, MAX_TOTAL_PIXELS - self.pixels)?;
        self.pixels += sheet.pixels.len();
        let id = SheetId(self.list.len() as u16);
        self.list.push(sheet);
        self.ids.insert(file.to_owned(), id);
        Ok(id)
    }

    fn size(&self, id: SheetId) -> [u32; 2] {
        let sheet = &self.list[usize::from(id.0)];
        [sheet.width, sheet.height]
    }
}

fn asset(file: &str, reason: impl ToString) -> AgentError {
    AgentError::Asset {
        file: file.to_owned(),
        reason: reason.to_string(),
    }
}

fn decode(file: &str, bytes: &[u8], budget: usize) -> Result<Sheet, AgentError> {
    let broken = |e: png::DecodingError| asset(file, format!("не PNG: {e}"));
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(broken)?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(asset(file, "лист больше 4096×4096"));
    }
    if width as usize * height as usize > budget {
        return Err(asset(file, "листы суммарно больше 64 МиБ"));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| asset(file, "слишком большой лист"))?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer).map_err(broken)?;
    let (color, _) = reader.output_color_type();
    let data = &buffer[..info.buffer_size()];
    let pixels = match color {
        ColorType::Grayscale => data.iter().map(|&g| pack(g, g, g, 255)).collect(),
        ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[g, a]| pack(g, g, g, a))
            .collect(),
        ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&[r, g, b]| pack(r, g, b, 255))
            .collect(),
        ColorType::Rgba => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| pack(r, g, b, a))
            .collect(),
        ColorType::Indexed => return Err(asset(file, "палитра не развёрнута")),
    };
    Ok(Sheet {
        width,
        height,
        pixels,
    })
}

fn pack(r: u8, g: u8, b: u8, a: u8) -> u32 {
    let premultiplied = |c: u8| (u32::from(c) * u32::from(a) + 127) / 255;
    (u32::from(a) << 24) | (premultiplied(r) << 16) | (premultiplied(g) << 8) | premultiplied(b)
}

fn build_clip(
    manifest: &Manifest,
    name: &str,
    animation: &Animation,
    sheet: Option<(SheetId, [u32; 2])>,
) -> Result<Clip, AgentError> {
    let bad = |reason: String| fail(format!("анимация «{name}»: {reason}"));
    // У тела листа нет: кадры лишь делят время клипа
    let (sheet, sheet_size) = sheet.unwrap_or((SheetId(0), [u32::MAX; 2]));
    let frames = match &animation.frames {
        Frames::Grid(grid) => {
            let [frame_w, frame_h] = manifest.frame_size;
            if grid.count == 0 || grid.count > MAX_FRAMES {
                return Err(bad("кадров должно быть от 1 до 256".into()));
            }
            if grid.duration_ms == 0 {
                return Err(bad("длительность кадра меньше 1 мс".into()));
            }
            let columns = grid.columns.unwrap_or(grid.count).min(grid.count);
            if columns == 0 {
                return Err(bad("columns не может быть нулевым".into()));
            }
            let rows = grid.count.div_ceil(columns);
            let fits = u64::from(columns) * u64::from(frame_w) <= u64::from(sheet_size[0])
                && u64::from(rows) * u64::from(frame_h) <= u64::from(sheet_size[1]);
            if !fits {
                return Err(bad("сетка кадров не помещается в лист".into()));
            }
            (0..grid.count)
                .map(|i| Frame {
                    sheet,
                    rect: [
                        (i % columns) * frame_w,
                        (i / columns) * frame_h,
                        frame_w,
                        frame_h,
                    ],
                    duration: seconds(grid.duration_ms),
                    offset: Vec2::ZERO,
                })
                .collect()
        }
        Frames::List(list) => {
            if list.is_empty() || list.len() > MAX_FRAMES as usize {
                return Err(bad("кадров должно быть от 1 до 256".into()));
            }
            let mut frames = Vec::with_capacity(list.len());
            for (index, r) in list.iter().enumerate() {
                if r.duration_ms == 0 {
                    return Err(bad(format!("кадр {index}: длительность меньше 1 мс")));
                }
                let empty = r.w == 0 || r.h == 0;
                let inside = u64::from(r.x) + u64::from(r.w) <= u64::from(sheet_size[0])
                    && u64::from(r.y) + u64::from(r.h) <= u64::from(sheet_size[1]);
                if empty || !inside {
                    return Err(bad(format!(
                        "кадр {index}: прямоугольник вне листа или пуст"
                    )));
                }
                manifest::check_point("offset", r.offset)?;
                frames.push(Frame {
                    sheet,
                    rect: [r.x, r.y, r.w, r.h],
                    duration: seconds(r.duration_ms),
                    offset: Vec2::new(r.offset[0], r.offset[1]),
                });
            }
            frames
        }
    };

    let mut markers = Vec::with_capacity(animation.events.len());
    for event in &animation.events {
        if event.frame as usize >= frames.len() {
            return Err(bad(format!(
                "событие «{}»: кадра {} нет",
                event.name, event.frame
            )));
        }
        markers.push(Marker {
            frame: event.frame as u16,
            name: event.name.clone(),
        });
    }
    markers.sort_by_key(|marker| marker.frame);

    let anchor = animation.anchor.unwrap_or(manifest.anchor);
    Ok(Clip {
        name: name.to_owned(),
        frames,
        looping: animation.looping,
        anchor: Vec2::new(anchor[0], anchor[1]),
        markers,
    })
}

fn seconds(ms: u32) -> f32 {
    ms as f32 / 1000.0
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
        writer.finish().unwrap();
        out
    }

    pub(crate) fn character(
        text: &str,
        files: &[(&str, Vec<u8>)],
    ) -> Result<Character, AgentError> {
        Character::from_source(text, &|file| {
            files
                .iter()
                .find(|(name, _)| *name == file)
                .map(|(_, bytes)| bytes.clone())
                .ok_or_else(|| io::ErrorKind::NotFound.into())
        })
    }

    /// Манифест полного спрайтового набора: циклы `loop`/`step` и разовая `once`.
    pub(crate) const PACK: &str = r#"{
        "id": "t", "frame_size": [4, 4], "anchor": [2, 4], "scale": 1.5,
        "animations": {
            "loop": { "file": "s.png", "loop": true, "frames": { "count": 4, "duration_ms": 100 } },
            "step": { "file": "s.png", "loop": true, "frames": { "count": 4, "duration_ms": 80 },
                      "events": [ { "frame": 1, "name": "step" }, { "frame": 3, "name": "step" } ] },
            "once": { "file": "s.png", "frames": { "count": 4, "duration_ms": 70 } }
        },
        "actions": {
            "spawn": { "animation": "once", "priority": 3, "interruptible": false },
            "idle": { "animation": "loop" },
            "walk": { "animation": "step" },
            "run": { "animation": "step" },
            "turn": { "animation": "once", "priority": 1 },
            "look": { "animation": "once" },
            "react": { "animation": "once", "priority": 2 },
            "interact": { "animation": "once", "priority": 2 },
            "celebrate": { "animation": "once", "priority": 5, "interruptible": false },
            "think": { "animation": "loop" },
            "busy": { "animation": "loop" },
            "warning": { "animation": "once", "priority": 4, "interruptible": false },
            "error": { "animation": "once", "priority": 4, "interruptible": false },
            "sleep": { "animation": "loop" },
            "wake_up": { "animation": "once", "priority": 2, "interruptible": false },
            "dragged": { "animation": "loop", "priority": 6 },
            "despawn": { "animation": "once", "priority": 7, "interruptible": false },
            "wave": { "animation": "once", "priority": 1 }
        },
        "motion": { "walk_speed": 70, "run_speed": 230 },
        "behavior": { "nap_weight": 0.3 }
    }"#;

    /// Лист PACK: четыре кадра 4×4 разных цветов.
    pub(crate) fn pack_sheet() -> Vec<u8> {
        let mut rgba = Vec::with_capacity(16 * 4 * 4);
        for _ in 0..4 {
            for frame in 0..4u8 {
                rgba.extend([60 * frame + 40, 200, 90, 255].repeat(4));
            }
        }
        png(16, 4, &rgba)
    }

    pub(crate) fn sprite_character() -> Character {
        character(PACK, &[("s.png", pack_sheet())]).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{PACK, character, pack_sheet, png};
    use super::*;
    use crate::action::names;

    // Лист 4×2 — два кадра 2×2.
    const BASE: &str = r#"{
        "id": "t", "frame_size": [2, 2], "anchor": [1, 2],
        "animations": { "a": { "file": "a.png", "frames": { "count": 2, "duration_ms": 100 } } },
        "actions": { "idle": { "animation": "a" }, "walk": { "animation": "a" } }
    }"#;

    fn load(text: &str) -> Result<Character, AgentError> {
        character(text, &[("a.png", png(4, 2, &[255; 32]))])
    }

    fn load_bytes(bytes: Vec<u8>) -> Result<Character, AgentError> {
        character(BASE, &[("a.png", bytes)])
    }

    fn manifest_error(text: &str) -> bool {
        matches!(load(text), Err(AgentError::Manifest(_)))
    }

    #[test]
    fn base_pack_loads() {
        let character = load(BASE).unwrap();
        let clip = character.clip(ClipId(0));
        assert_eq!(clip.frames[1].rect, [2, 0, 2, 2]);
        assert_eq!(clip.anchor, Vec2::new(1.0, 2.0));
        assert_eq!(character.scale(), 1.0);
    }

    #[test]
    fn builtin_packs_have_every_action_but_turn() {
        for kind in RigKind::ALL {
            builtin_has_every_action_but_turn(kind);
        }
    }

    fn builtin_has_every_action_but_turn(kind: RigKind) {
        let character = Character::builtin(kind).unwrap();
        for name in [
            names::SPAWN,
            names::IDLE,
            names::WALK,
            names::RUN,
            names::LOOK,
            names::REACT,
            names::INTERACT,
            names::CELEBRATE,
            names::THINK,
            names::BUSY,
            names::WARNING,
            names::ERROR,
            names::SLEEP,
            names::WAKE_UP,
            names::DRAGGED,
            names::DESPAWN,
            "wave",
        ] {
            assert!(character.action_id(name).is_some(), "{name}");
        }
        let set = character.actions();
        assert!(set.turn.is_none() && set.wake_up.is_some() && set.despawn.is_some());
        assert_eq!(character.rig(), Some(kind));
        assert_eq!(character.id(), kind.name());
        assert!(character.sheets.is_empty());
        let walk = character.clip(character.action(set.walk).clip);
        assert!(walk.looping && (walk.duration() - 0.8).abs() < 1e-4);
    }

    #[test]
    fn sprite_pack_loads_from_directory() {
        let dir = std::env::temp_dir().join(format!("ui-agent-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("agent.json"), PACK).unwrap();
        std::fs::write(dir.join("s.png"), pack_sheet()).unwrap();
        let loaded = Character::load_dir(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        let pack = loaded.unwrap();
        assert_eq!(pack.rig(), None);
        let set = pack.actions();
        assert!(set.turn.is_some() && set.wake_up.is_some() && set.despawn.is_some());
        let walk = pack.clip(pack.action(set.walk).clip);
        assert!(walk.looping && walk.markers.iter().all(|m| m.name == "step"));
        assert!(pack.sheet(SheetId(0)).pixels.iter().any(|p| p >> 24 != 0));
    }

    const RIG: &str = r#"{
        "id": "t", "rig": "goose", "frame_size": [2, 2], "anchor": [1, 2],
        "animations": { "a": { "frames": { "count": 2, "duration_ms": 100 } } },
        "actions": { "idle": { "animation": "a" }, "walk": { "animation": "a" } }
    }"#;

    #[test]
    fn rig_manifest_needs_no_sheets() {
        let character = character(RIG, &[]).unwrap();
        assert_eq!(character.rig(), Some(RigKind::Goose));
        assert!((character.clip(ClipId(0)).duration() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn rig_manifest_rejects_file_and_sprite_manifest_requires_it() {
        let with_file = RIG.replace("\"frames\"", "\"file\": \"a.png\", \"frames\"");
        assert!(manifest_error(&with_file));
        let without = BASE.replace("\"file\": \"a.png\", ", "");
        assert!(manifest_error(&without));
        let list = RIG.replace(
            "{ \"count\": 2, \"duration_ms\": 100 }",
            "[ { \"x\": 0, \"y\": 0, \"w\": 1, \"h\": 1, \"duration_ms\": 50 } ]",
        );
        assert!(manifest_error(&list));
    }

    #[test]
    fn unknown_rig_is_rejected() {
        assert!(manifest_error(&RIG.replace("goose", "swan")));
    }

    #[test]
    fn missing_directory_is_an_error() {
        assert!(Character::load_dir(Path::new("no/such/pack")).is_err());
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(manifest_error("{"));
        assert!(manifest_error(
            &BASE.replace("\"id\"", "\"extra\": 1, \"id\"")
        ));
        assert!(manifest_error(
            &BASE.replace("\"count\": 2", "\"count\": 3")
        ));
        assert!(manifest_error(
            &BASE.replace("\"count\": 2", "\"count\": 257")
        ));
        assert!(manifest_error(
            &BASE.replace("\"duration_ms\": 100", "\"duration_ms\": 0")
        ));
        assert!(manifest_error(&BASE.replace("[2, 2]", "[0, 2]")));
        assert!(manifest_error(
            &BASE.replace("\"id\"", "\"scale\": 0, \"id\"")
        ));
        assert!(manifest_error(
            &BASE.replace("\"id\"", "\"scale\": 17, \"id\"")
        ));
        assert!(manifest_error(&BASE.replace(
            "\"animation\": \"a\" }, \"walk\"",
            "\"animation\": \"zz\" }, \"walk\""
        )));
        assert!(manifest_error(&BASE.replace("\"idle\"", "\"stand\"")));
        assert!(manifest_error(&BASE.replace("\"walk\"", "\"stroll\"")));
        assert!(manifest_error(&format!(
            "{BASE}{}",
            " ".repeat(MAX_MANIFEST_BYTES)
        )));
    }

    #[test]
    fn rejects_bad_frame_lists() {
        let list = |frame: &str| {
            BASE.replace(
                "{ \"count\": 2, \"duration_ms\": 100 }",
                &format!("[ {frame} ]"),
            )
        };
        let good = r#"{ "x": 2, "y": 0, "w": 2, "h": 2, "duration_ms": 50, "offset": [1, -1] }"#;
        assert_eq!(
            load(&list(good)).unwrap().clip(ClipId(0)).frames[0].offset,
            Vec2::new(1.0, -1.0)
        );
        for bad in [
            r#"{ "x": 3, "y": 0, "w": 2, "h": 2, "duration_ms": 50 }"#,
            r#"{ "x": 0, "y": 1, "w": 2, "h": 2, "duration_ms": 50 }"#,
            r#"{ "x": 0, "y": 0, "w": 0, "h": 2, "duration_ms": 50 }"#,
            r#"{ "x": 0, "y": 0, "w": 2, "h": 2, "duration_ms": 0 }"#,
            r#"{ "x": 4294967295, "y": 0, "w": 2, "h": 2, "duration_ms": 50 }"#,
        ] {
            assert!(manifest_error(&list(bad)), "{bad}");
        }
        assert!(manifest_error(&list("")));
    }

    #[test]
    fn rejects_bad_events() {
        let with = |frame: u32| {
            BASE.replace(
                "\"frames\"",
                &format!("\"events\": [{{ \"frame\": {frame}, \"name\": \"x\" }}], \"frames\""),
            )
        };
        assert!(load(&with(1)).is_ok());
        assert!(manifest_error(&with(2)));
    }

    #[test]
    fn rejects_unsafe_paths() {
        for file in [
            "../a.png", "/a.png", "C:/a.png", "C:a.png", "x\\a.png", "x//a.png", "./a.png", "",
        ] {
            let text = BASE.replace("\"a.png\"", &format!("\"{}\"", file.replace('\\', "\\\\")));
            assert!(manifest_error(&text), "{file}");
        }
    }

    #[test]
    fn rejects_unreadable_sprites() {
        let missing = BASE.replace("\"a.png\"", "\"b.png\"");
        assert!(matches!(load(&missing), Err(AgentError::Asset { .. })));
        assert!(matches!(
            load_bytes(b"not a png".to_vec()),
            Err(AgentError::Asset { .. })
        ));
        let mut cut = png(4, 2, &[255; 32]);
        cut.truncate(cut.len() / 2);
        assert!(matches!(load_bytes(cut), Err(AgentError::Asset { .. })));
    }

    #[test]
    fn rejects_oversized_sheets() {
        let wide = png(4097, 1, &vec![0; 4097 * 4]);
        assert!(matches!(load_bytes(wide), Err(AgentError::Asset { .. })));
    }

    #[test]
    fn shared_sheet_is_decoded_once() {
        let text = BASE.replace(
            "\"a\": {",
            "\"b\": { \"file\": \"a.png\", \"frames\": { \"count\": 1, \"duration_ms\": 10 } }, \"a\": {",
        );
        let character = load(&text).unwrap();
        assert_eq!(character.sheets.len(), 1);
        assert_eq!(character.clips.len(), 2);
    }

    #[test]
    fn palette_and_16_bit_sheets_are_normalised() {
        let mut indexed = Vec::new();
        let mut encoder = png::Encoder::new(&mut indexed, 4, 2);
        encoder.set_color(ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![10, 20, 30]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0; 8]).unwrap();
        writer.finish().unwrap();
        let loaded = load_bytes(indexed).unwrap();
        assert_eq!(loaded.sheet(SheetId(0)).pixels[7], 0xFF0A_141E);

        let mut grey = Vec::new();
        let mut encoder = png::Encoder::new(&mut grey, 4, 2);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0x80; 16]).unwrap();
        writer.finish().unwrap();
        let loaded = load_bytes(grey).unwrap();
        assert_eq!(loaded.sheet(SheetId(0)).pixels[0], 0xFF80_8080);
    }

    #[test]
    fn pixels_are_premultiplied() {
        let sheet = png(4, 2, &[100, 50, 200, 128].repeat(8));
        let character = character(BASE, &[("a.png", sheet)]).unwrap();
        assert_eq!(
            character.sheet(SheetId(0)).pixels[0],
            (128 << 24) | (50 << 16) | (25 << 8) | 100
        );
    }
}
