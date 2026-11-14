//! Чистая отрисовка сцены на холст: персонаж, следы с частицами и карточка.
//! Координаты мира — физические пиксели; холст знает лишь свой левый верхний угол.

use std::sync::Arc;

use super::shadow::{self, Oval};
use super::{Canvas, Color, Figure, Sprite, marks, pixel, props, puddle};
use crate::animation::SheetId;
use crate::command::Tone;
use crate::effects::{
    footprints::Footprints,
    leaves::{Leaves, SHADOW_GROW},
    mud::Mud,
    particles::Particles,
    props::Props,
};
use crate::math::{Rect, Vec2};
use crate::rig::{Creature, Pose, Skeleton};

/// Окно в целых пикселях виртуального экрана.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Наименьший целый прямоугольник, накрывающий `rect`.
pub fn enclosing(rect: Rect) -> PixelRect {
    let (x, y) = (rect.min.x.floor() as i32, rect.min.y.floor() as i32);
    let span = |from: i32, to: f32| ((to.ceil() as i32).saturating_sub(from)).max(1) as u32;
    PixelRect {
        x,
        y,
        width: span(x, rect.max.x),
        height: span(y, rect.max.y),
    }
}

// --- персонаж ---

/// Что рисовать: кадр спрайта или поза процедурного тела.
#[derive(Clone, Copy)]
pub enum CharacterView<'a> {
    Sprite(SpriteView<'a>),
    /// `extent` — половина стороны холста вокруг точки на земле.
    Creature {
        pose: &'a Pose,
        extent: f32,
        /// Скелет набора: по нему тело рисуется.
        skeleton: &'a Arc<Skeleton>,
    },
}

/// Кадр персонажа и его положение; `scale` уже включает масштаб набора.
#[derive(Clone, Copy)]
pub struct SpriteView<'a> {
    pub sheet: SheetId,
    pub sprite: Sprite<'a>,
    pub anchor: Vec2,
    pub offset: Vec2,
    /// Точка опоры (ноги) в мире.
    pub pos: Vec2,
    pub flip: bool,
    pub scale: f32,
}

/// Зеркалит вокруг якоря: на экране якорь остаётся на `pos` при любой стороне.
pub fn character_rect(view: &SpriteView) -> PixelRect {
    let [_, _, w, h] = view.sprite.rect;
    let (width, height) = (w as f32 * view.scale, h as f32 * view.scale);
    let from_anchor = (view.anchor.x - view.offset.x) * view.scale;
    let left = if view.flip {
        view.pos.x + from_anchor - width
    } else {
        view.pos.x - from_anchor
    };
    let top = view.pos.y - (view.anchor.y - view.offset.y) * view.scale;
    PixelRect {
        x: left.round() as i32,
        y: top.round() as i32,
        width: width.ceil().max(1.0) as u32,
        height: height.ceil().max(1.0) as u32,
    }
}

/// Холст — ровно `character_rect`.
pub fn draw_character(canvas: &mut Canvas, view: &SpriteView) {
    canvas.blit(view.sprite, Vec2::ZERO, view.scale, view.flip, 1.0);
}

// --- следы и частицы ---

/// Сторона окна эффектов не больше этого, px.
const EFFECTS_MAX_SIDE: f32 = 2048.0;
/// След — кружок размером с лапку, логические px; у тела свой размер отпечатка.
pub const PRINT_RADIUS: f32 = 2.9;
const PRINT_OPACITY: f32 = 0.35;
const PRINT_COLOR: Color = Color(0x20_20_28);
/// Грязный след жирнее и крупнее обычного: копыто несёт комок грязи.
const MUD_PRINT_OPACITY: f32 = 0.8;
const MUD_PRINT_GROW: f32 = 1.15;
const MUD_PRINT: Color = Color(0x5E_3D_22);
/// Каракули клювом: мел на рабочем столе, логические px.
const SCRIBBLE: Color = Color(0x4A_55_65);
const SCRIBBLE_RADIUS: f32 = 1.6;

/// Не уже, чем даёт мягкий край круга.
const EDGE: f32 = 1.0;

fn cap_axis(lo: f32, hi: f32, focus: f32) -> (f32, f32) {
    if hi - lo <= EFFECTS_MAX_SIDE {
        return (lo, hi);
    }
    let start = (focus - EFFECTS_MAX_SIDE * 0.5).clamp(lo, hi - EFFECTS_MAX_SIDE);
    (start, start + EFFECTS_MAX_SIDE)
}

/// Всё, что лежит в окне эффектов: под персонажем и насквозь для мыши.
#[derive(Clone, Copy)]
pub struct Effects<'a> {
    pub footprints: &'a Footprints,
    /// Радиус отпечатка, логические px.
    pub print_radius: f32,
    pub particles: &'a Particles,
    pub leaves: &'a Leaves,
    pub mud: &'a Mud,
    /// Грязные следы свинки после лужи: живут дольше обычных.
    pub mud_prints: &'a Footprints,
    pub props: &'a Props,
    /// Линия, которую гусь рисует клювом.
    pub scribble: &'a Footprints,
    /// Гусята: позы малышей, идущих за гусем, и скелет, по которому они рисуются.
    pub flock: &'a [Pose],
    pub flock_rig: Option<&'a Arc<Skeleton>>,
}

/// Охват эффектов; при разбросе больше предела окно держится у самого свежего.
pub fn effects_rect(effects: &Effects<'_>, ui_scale: f32) -> Option<Rect> {
    let Effects {
        footprints,
        print_radius,
        particles,
        leaves,
        mud,
        mud_prints,
        props,
        scribble,
        flock,
        flock_rig: _,
    } = *effects;
    let reach = (print_radius + EDGE) * ui_scale;
    // Мозаика размазывает край на клетку
    let bleed = EDGE + pixel::cell(ui_scale) as f32;
    let flock = flock
        .iter()
        .map(|pose| Rect::new(pose.pos, pose.pos).inflate(Creature::extent(pose.scale)))
        .reduce(Rect::union);
    let all = [
        footprints.bounds(reach),
        mud_prints.bounds(reach * MUD_PRINT_GROW),
        scribble.bounds((SCRIBBLE_RADIUS + EDGE) * ui_scale),
        particles.bounds().map(|rect| rect.inflate(bleed)),
        leaves.bounds().map(|rect| rect.inflate(EDGE)),
        mud.bounds().map(|rect| rect.inflate(bleed)),
        props.bounds().map(|rect| rect.inflate(bleed)),
        flock,
    ]
    .into_iter()
    .flatten()
    .reduce(Rect::union)?;
    let focus = footprints
        .iter()
        .last()
        .or_else(|| mud_prints.iter().last())
        .map(|print| print.pos)
        .or_else(|| particles.iter().last().map(|particle| particle.pos))
        .unwrap_or_else(|| all.center());
    let (min_x, max_x) = cap_axis(all.min.x, all.max.x, focus.x);
    let (min_y, max_y) = cap_axis(all.min.y, all.max.y, focus.y);
    Some(Rect::new(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y)))
}

/// `origin` — мировая точка левого верхнего угла холста; `figure` рисует гусят и живёт
/// между кадрами.
pub fn draw_effects(
    canvas: &mut Canvas,
    origin: Vec2,
    effects: &Effects<'_>,
    ui_scale: f32,
    figure: &mut Figure,
) {
    // Лужа и грязь лежат на земле ниже всего
    let cell = pixel::cell(ui_scale);
    puddle::draw(canvas, origin, effects.mud, cell);
    for print in effects.mud_prints.iter() {
        canvas.fill_circle(
            print.pos - origin,
            effects.print_radius * MUD_PRINT_GROW * ui_scale,
            MUD_PRINT,
            print.opacity * MUD_PRINT_OPACITY,
        );
    }
    for mark in effects.scribble.iter() {
        let radius = SCRIBBLE_RADIUS * ui_scale;
        canvas.fill_circle(mark.pos - origin, radius, SCRIBBLE, mark.opacity);
    }
    // Тени — первыми: листья горки висят над своими тенями, и куча читается объёмной
    props::shadows(canvas, origin, effects.props, cell);
    for leaf in effects.leaves.iter() {
        let spot = Oval {
            center: leaf.pos,
            radii: leaf.radii * SHADOW_GROW,
            rotation: leaf.angle,
        };
        shadow::dotted(canvas, origin, &spot, cell, shadow::OPACITY * leaf.opacity);
    }
    for leaf in effects.leaves.iter() {
        let center = leaf.pos - origin - Vec2::new(0.0, leaf.lift);
        let color = Color(leaf.color);
        canvas.fill_ellipse(center, leaf.radii, leaf.angle, color, leaf.opacity);
    }
    for print in effects.footprints.iter() {
        let opacity = print.opacity * PRINT_OPACITY;
        canvas.fill_circle(
            print.pos - origin,
            effects.print_radius * ui_scale,
            PRINT_COLOR,
            opacity,
        );
    }
    props::draw(canvas, origin, effects.props);
    for (pose, rig) in effects
        .flock
        .iter()
        .zip(effects.flock_rig.into_iter().cycle())
    {
        figure.paint(canvas, origin, pose, rig);
    }
    for particle in effects.particles.iter() {
        marks::draw(canvas, origin, &particle);
    }
    pixel::retro(canvas, origin, ui_scale);
}

// --- карточка ---

/// Размеры карточки в логических px.
pub const CARD_WIDTH: f32 = 260.0;
pub const TITLE_PX: f32 = 15.0;
pub const BODY_PX: f32 = 13.0;
const CARD_MAX_HEIGHT: f32 = 156.0;
const CARD_MIN_HEIGHT: f32 = 44.0;
const PADDING: f32 = 12.0;
/// Метка тона — кружок, как лапки гуся.
const DOT_RADIUS: f32 = 5.0;
const TEXT_LEFT: f32 = PADDING + 2.0 * DOT_RADIUS + 8.0;
const TITLE_LINE: f32 = 22.0;
const TEXT_GAP: f32 = 4.0;
const CORNER: f32 = 5.0;
/// Высота броска карточки и глубина её тени на земле.
const CARD_TOSS: f32 = 34.0;
const CARD_SHADOW_DEPTH: f32 = 8.0;
/// Тень уже карточки и бледнеет, пока та в воздухе.
const CARD_SHADOW_WIDTH: f32 = 0.46;
const CARD_SHADOW_FADE: f32 = 0.45;

/// Те же белый, обводка и чернила, что у гуся.
const BORDER: Color = Color(0xB9_C0_CB);
const PANEL: Color = Color(0xFF_FF_FF);
const TITLE_INK: Color = Color(0x1B_1F_29);
const BODY_INK: Color = Color(0x4A_55_65);

/// Растровый текст: покрытие `width`×`height`.
#[derive(Clone, Copy)]
pub struct Mask<'a> {
    pub coverage: &'a [u8],
    pub width: u32,
    pub height: u32,
}

fn tone_color(tone: Tone) -> Color {
    Color(match tone {
        Tone::Info => 0x3D_8B_D9,
        Tone::Success => 0x4F_A8_5A,
        Tone::Warning => 0xFF_9A_1F,
        Tone::Error => 0xD9_44_2B,
    })
}

/// Предельная ширина текста, px.
pub fn text_width(ui_scale: f32) -> u32 {
    ((CARD_WIDTH - TEXT_LEFT - PADDING) * ui_scale) as u32
}

/// Предельная высота заголовка (одна строка), px.
pub fn title_limit(ui_scale: f32) -> u32 {
    (TITLE_LINE * ui_scale) as u32
}

/// Сколько высоты остаётся тексту под заголовком, px.
pub fn body_limit(title_height: u32, ui_scale: f32) -> u32 {
    let room = ((CARD_MAX_HEIGHT - 2.0 * PADDING) * ui_scale) as u32;
    let used = if title_height > 0 {
        title_height + (TEXT_GAP * ui_scale) as u32
    } else {
        0
    };
    room.saturating_sub(used)
}

fn text_height(title: Option<u32>, body: Option<u32>, ui_scale: f32) -> f32 {
    let gap = if title.is_some() && body.is_some() {
        TEXT_GAP * ui_scale
    } else {
        0.0
    };
    (title.unwrap_or(0) + body.unwrap_or(0)) as f32 + gap
}

/// Ширина фиксирована, высота растёт с текстом в пределах карточки.
pub fn card_size(title_height: Option<u32>, body_height: Option<u32>, ui_scale: f32) -> (u32, u32) {
    let content = text_height(title_height, body_height, ui_scale) + 2.0 * PADDING * ui_scale;
    let height = content.clamp(CARD_MIN_HEIGHT * ui_scale, CARD_MAX_HEIGHT * ui_scale);
    (
        (CARD_WIDTH * ui_scale).round() as u32,
        height.round() as u32,
    )
}

/// Окно карточки с нижним центром в `bottom_center` и поп-масштабом `scale`.
pub fn card_rect(bottom_center: Vec2, size: (u32, u32), scale: f32) -> PixelRect {
    let scale = if scale.is_finite() {
        scale.max(0.0)
    } else {
        0.0
    };
    let (width, height) = (size.0 as f32 * scale, size.1 as f32 * scale);
    PixelRect {
        x: (bottom_center.x - width * 0.5).round() as i32,
        y: (bottom_center.y - height).round() as i32,
        width: width.ceil() as u32,
        height: height.ceil() as u32,
    }
}

/// Где карточка на экране: окно, панель внутри него и тень на земле.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPlacement {
    pub window: PixelRect,
    /// Левый верхний угол панели в окне.
    pub panel: Vec2,
    /// Пятно под карточкой, мировые px.
    pub shadow: Oval,
    pub shadow_opacity: f32,
}

/// `ground` — точка на земле под нижним центром карточки, `toss` — высота броска 0..1.
/// Окно охватывает и панель в воздухе, и её тень на земле.
pub fn place_card(
    ground: Vec2,
    size: (u32, u32),
    pop: f32,
    toss: f32,
    ui_scale: f32,
) -> CardPlacement {
    let toss = if toss.is_finite() {
        toss.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let panel = card_rect(
        ground - Vec2::new(0.0, toss * CARD_TOSS * ui_scale),
        size,
        pop,
    );
    let shadow = Oval {
        center: ground,
        radii: Vec2::new(
            panel.width as f32 * CARD_SHADOW_WIDTH,
            // Пятно растёт вместе с «выскакивающей» карточкой
            CARD_SHADOW_DEPTH * ui_scale * panel.width as f32 / (size.0.max(1) as f32),
        ),
        rotation: 0.0,
    };
    let spot = Rect::new(ground - shadow.radii, ground + shadow.radii).inflate(EDGE);
    let top_left = Vec2::new(panel.x as f32, panel.y as f32);
    let size = Vec2::new(panel.width as f32, panel.height as f32);
    let window = enclosing(Rect::new(top_left, top_left + size).union(spot));
    CardPlacement {
        window,
        panel: top_left - Vec2::new(window.x as f32, window.y as f32),
        shadow,
        shadow_opacity: shadow::OPACITY * (1.0 - CARD_SHADOW_FADE * toss),
    }
}

/// Тень карточки; звать до панели.
pub fn draw_card_shadow(canvas: &mut Canvas, place: &CardPlacement, ui_scale: f32) {
    let origin = Vec2::new(place.window.x as f32, place.window.y as f32);
    let cell = pixel::cell(ui_scale);
    shadow::dotted(canvas, origin, &place.shadow, cell, place.shadow_opacity);
}

/// Холст — ровно `card_size`: белая панель с тонкой обводкой и кружком тона в общей
/// мозаике; текст кладётся поверх мозаики и остаётся чётким.
pub fn draw_card(
    canvas: &mut Canvas,
    tone: Tone,
    title: Option<Mask>,
    body: Option<Mask>,
    ui_scale: f32,
) {
    let (width, height) = (canvas.width() as f32, canvas.height() as f32);
    let border = ui_scale.round().max(1.0);
    let corner = CORNER * ui_scale;
    // Рамка — тёмная подложка чуть больше панели.
    canvas.fill_round_rect(
        Rect::from_xywh(0.0, 0.0, width, height),
        corner,
        BORDER,
        1.0,
    );
    let inner = Rect::from_xywh(border, border, width - 2.0 * border, height - 2.0 * border);
    canvas.fill_round_rect(inner, corner - border, PANEL, 1.0);

    let dot = Vec2::new((PADDING + DOT_RADIUS) * ui_scale, height * 0.5);
    canvas.fill_circle(dot, DOT_RADIUS * ui_scale, tone_color(tone), 1.0);
    pixel::pixelate(canvas, Vec2::ZERO, ui_scale);

    let content = text_height(title.map(|m| m.height), body.map(|m| m.height), ui_scale);
    let mut y = ((height - content) * 0.5).max(0.0);
    for (mask, ink) in [(title, TITLE_INK), (body, BODY_INK)] {
        let Some(mask) = mask else { continue };
        let top_left = Vec2::new(TEXT_LEFT * ui_scale, y);
        canvas.blit_mask(mask.coverage, mask.width, mask.height, top_left, ink, 1.0);
        y += mask.height as f32 + TEXT_GAP * ui_scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: u32 = 0xFF_FF_00_00;

    fn buffer(side: usize) -> Vec<u32> {
        vec![0; side * side]
    }

    fn canvas(buffer: &mut [u32], width: u32, height: u32) -> Canvas<'_> {
        Canvas::new(buffer, width as usize, width, height).unwrap()
    }

    /// Левая половина кадра 4×2 красная, правая прозрачная.
    const HALF: [u32; 8] = [RED, RED, 0, 0, RED, RED, 0, 0];

    fn view(pixels: &[u32], flip: bool, pos: Vec2) -> SpriteView<'_> {
        SpriteView {
            sheet: SheetId(0),
            sprite: Sprite {
                pixels,
                stride: 4,
                rect: [0, 0, 4, 2],
            },
            anchor: Vec2::new(1.0, 2.0),
            offset: Vec2::ZERO,
            pos,
            flip,
            scale: 2.0,
        }
    }

    /// Непрозрачные пиксели вне `[0, w) × [0, h)`.
    fn leaked(canvas: &Canvas, w: i32, h: i32) -> usize {
        (0..canvas.height() as i32)
            .flat_map(|y| (0..canvas.width() as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| (x >= w || y >= h) && canvas.alpha_at(x, y) != 0)
            .count()
    }

    #[test]
    fn character_stands_on_its_anchor() {
        let view = view(&HALF, false, Vec2::new(100.0, 200.0));
        assert_eq!(
            character_rect(&view),
            PixelRect {
                x: 98,
                y: 196,
                width: 8,
                height: 4
            }
        );
    }

    #[test]
    fn mirrored_character_keeps_anchor_and_flips_pixels() {
        let view = view(&HALF, true, Vec2::new(100.0, 200.0));
        let rect = character_rect(&view);
        // Якорь на 1 px слева в кадре 4 px: после отражения он на 3 px справа от левого края.
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (94, 196, 8, 4));
        let mut pixels = buffer(12);
        let mut canvas = canvas(&mut pixels, 12, 12);
        draw_character(&mut canvas, &view);
        assert_eq!(canvas.alpha_at(7, 0), 255);
        assert_eq!(canvas.alpha_at(0, 0), 0);
        assert_eq!(leaked(&canvas, 8, 4), 0);
    }

    #[test]
    fn character_draws_inside_its_rect_only() {
        let view = view(&HALF, false, Vec2::new(-50.3, -20.7));
        let rect = character_rect(&view);
        assert_eq!((rect.x, rect.y), (-52, -25));
        let mut pixels = buffer(16);
        let mut canvas = canvas(&mut pixels, 16, 16);
        draw_character(&mut canvas, &view);
        assert_eq!(canvas.alpha_at(0, 0), 255);
        assert_eq!(canvas.alpha_at(3, 3), 255);
        assert_eq!(canvas.alpha_at(4, 0), 0);
        assert_eq!(leaked(&canvas, rect.width as i32, rect.height as i32), 0);
    }

    struct Layer {
        prints: Footprints,
        particles: Particles,
        leaves: Leaves,
        mud: Mud,
        mud_prints: Footprints,
        props: Props,
        scribble: Footprints,
    }

    impl Layer {
        fn empty() -> Self {
            Self {
                prints: Footprints::new(8, 4.5, 20.0),
                particles: Particles::new(16),
                leaves: Leaves::new(16),
                mud: Mud::new(1),
                mud_prints: Footprints::new(8, 10.0, 20.0),
                props: Props::new(4),
                scribble: Footprints::new(8, 6.0, 3.0),
            }
        }

        fn view(&self) -> Effects<'_> {
            Effects {
                footprints: &self.prints,
                print_radius: PRINT_RADIUS,
                particles: &self.particles,
                leaves: &self.leaves,
                mud: &self.mud,
                mud_prints: &self.mud_prints,
                props: &self.props,
                scribble: &self.scribble,
                flock: &[],
                flock_rig: None,
            }
        }
    }

    fn effects(origin: Vec2) -> Layer {
        let mut layer = Layer::empty();
        let prints = &mut layer.prints;
        for step in 0..4 {
            prints.track(
                origin + Vec2::new(step as f32 * 25.0, 0.0),
                Vec2::new(1.0, 0.0),
                4.0,
                true,
            );
        }
        let mut rng = crate::rng::Rng::new(5);
        layer.particles.burst(
            crate::effects::particles::Burst::Sparks,
            origin + Vec2::new(40.0, -30.0),
            1.0,
            &mut rng,
        );
        layer.leaves.heap(
            origin + Vec2::new(90.0, 40.0),
            20.0,
            12,
            10.0,
            1.0,
            &mut rng,
        );
        layer.leaves.update(1.0);
        layer
            .mud
            .pour(origin + Vec2::new(-60.0, 50.0), 30.0, 10.0, &mut rng);
        layer.mud.update(1.0);
        layer.props.spawn(
            crate::effects::props::PropKind::Ball,
            origin + Vec2::new(130.0, -20.0),
            15.0,
            1.0,
            10.0,
        );
        layer.mud_prints.stamp(
            origin + Vec2::new(-20.0, 60.0),
            Vec2::new(1.0, 0.0),
            true,
            0.0,
            1.0,
        );
        layer
    }

    #[test]
    fn effects_stay_inside_their_rect_including_negative_world() {
        for origin in [Vec2::new(300.0, 400.0), Vec2::new(-800.0, -300.0)] {
            let layer = effects(origin);
            let all = layer.view();
            let rect = enclosing(effects_rect(&all, 1.0).unwrap());
            let mut pixels = buffer(400);
            let mut canvas = canvas(&mut pixels, 400, 400);
            let from = Vec2::new(rect.x as f32, rect.y as f32);
            draw_effects(&mut canvas, from, &all, 1.0, &mut Figure::default());
            let drawn = (0..rect.height as i32)
                .flat_map(|y| (0..rect.width as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| canvas.alpha_at(x, y) != 0)
                .count();
            assert!(drawn > 0);
            assert_eq!(leaked(&canvas, rect.width as i32, rect.height as i32), 0);
            let mud = pixels
                .iter()
                .filter(|p| **p == 0xFF00_0000 | puddle::MUD_TONES[1].0)
                .count();
            assert!(mud > 100, "{mud}");
        }
    }

    #[test]
    fn empty_effects_have_no_rect() {
        assert!(effects_rect(&Layer::empty().view(), 1.0).is_none());
    }

    #[test]
    fn huge_spread_is_capped_around_the_newest_print() {
        let mut layer = Layer::empty();
        let prints = &mut layer.prints;
        for x in [0.0, 30.0, 6000.0, 6030.0] {
            prints.track(Vec2::new(x, 0.0), Vec2::new(1.0, 0.0), 4.0, true);
            prints.track(Vec2::new(x + 25.0, 0.0), Vec2::new(1.0, 0.0), 4.0, true);
            prints.track(Vec2::new(x, 0.0), Vec2::ZERO, 4.0, false);
        }
        let rect = effects_rect(&layer.view(), 1.0).unwrap();
        let newest = layer.prints.iter().last().unwrap().pos;
        assert!(rect.width() <= EFFECTS_MAX_SIDE && rect.height() <= EFFECTS_MAX_SIDE);
        assert!(rect.contains(newest));
    }

    fn solid(width: u32, height: u32) -> Vec<u8> {
        vec![255; (width * height) as usize]
    }

    fn mask(coverage: &[u8], width: u32, height: u32) -> Mask<'_> {
        Mask {
            coverage,
            width,
            height,
        }
    }

    #[test]
    fn card_size_grows_with_text_within_limits() {
        assert_eq!(card_size(None, None, 1.0), (260, 44));
        assert_eq!(card_size(Some(20), Some(34), 1.0), (260, 82));
        assert_eq!(card_size(Some(22), Some(500), 1.0), (260, 156));
        assert_eq!(card_size(Some(20), None, 2.0), (520, 88));
        assert_eq!(body_limit(22, 1.0), 132 - 22 - 4);
        assert_eq!(body_limit(0, 1.0), 132);
    }

    #[test]
    fn card_has_tone_dot_panel_text_and_round_corners() {
        let (title, body) = (solid(40, 20), solid(60, 30));
        let (width, height) = card_size(Some(20), Some(30), 1.0);
        for (tone, rgb) in [
            (Tone::Info, 0x3D8BD9),
            (Tone::Success, 0x4FA85A),
            (Tone::Warning, 0xFF9A1F),
            (Tone::Error, 0xD9442B),
        ] {
            let mut pixels = vec![0; (width * height) as usize];
            let mut canvas = canvas(&mut pixels, width, height);
            draw_card(
                &mut canvas,
                tone,
                Some(mask(&title, 40, 20)),
                Some(mask(&body, 60, 30)),
                1.0,
            );
            // Углы скруглены; мозаика оставляет в угловой клетке лишь след соседей
            assert!(canvas.alpha_at(0, 0) < 64);
            assert!(canvas.alpha_at(width as i32 - 1, height as i32 - 1) < 64);
            let middle = (height / 2) as usize * width as usize;
            assert_eq!(pixels[middle + 17], 0xFF00_0000 | rgb);
            assert_eq!(pixels[middle + 250], 0xFFFF_FFFF);
            // Обводка в пиксель смягчена мозаикой, но остаётся серее панели
            assert!(pixels[middle] >> 24 == 0xFF && pixels[middle] & 0xFF < 0xF4);
            // Текст лежит поверх мозаики нетронутым
            assert_eq!(pixels[16 * width as usize + 34], 0xFF1B_1F29);
        }
    }

    #[test]
    fn tossed_card_window_holds_both_the_panel_and_its_ground_shadow() {
        let (size, ground) = ((260, 80), Vec2::new(500.0, 300.0));
        let resting = place_card(ground, size, 1.0, 0.0, 1.0);
        let flying = place_card(ground, size, 1.0, 1.0, 1.0);
        assert_eq!(resting.panel.y, 0.0);
        assert_eq!(flying.window.y, resting.window.y - 34);
        assert_eq!(flying.window.height, resting.window.height + 34);
        assert!(flying.shadow_opacity < resting.shadow_opacity);
        for place in [resting, flying] {
            let bottom = place.window.y + place.window.height as i32;
            assert!(bottom as f32 >= ground.y + place.shadow.radii.y);
            assert_eq!(place.shadow.center, ground);
            let mut pixels = vec![0; (place.window.width * place.window.height) as usize];
            let mut canvas = canvas(&mut pixels, place.window.width, place.window.height);
            draw_card_shadow(&mut canvas, &place, 1.0);
            assert!(pixels.iter().any(|p| *p != 0));
        }
        assert_eq!(place_card(ground, size, 1.0, f32::NAN, 1.0), resting);
    }

    #[test]
    fn card_rect_is_anchored_at_bottom_center_and_pops() {
        let size = (260, 80);
        assert_eq!(
            card_rect(Vec2::new(500.0, 300.0), size, 1.0),
            PixelRect {
                x: 370,
                y: 220,
                width: 260,
                height: 80
            }
        );
        let half = card_rect(Vec2::new(-100.0, 0.0), size, 0.5);
        assert_eq!(
            (half.x, half.y, half.width, half.height),
            (-165, -40, 130, 40)
        );
        assert_eq!(card_rect(Vec2::ZERO, size, 0.0).width, 0);
        assert_eq!(card_rect(Vec2::ZERO, size, f32::NAN).height, 0);
    }
}
