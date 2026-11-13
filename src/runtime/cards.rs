//! Окна карточек: по окну на живую карточку, панель рисуется один раз.

use std::collections::{HashMap, hash_map::Entry};

use crate::command::Notification;
use crate::effects::items::{Item, ItemId, Items};
use crate::error::AgentError;
use crate::platform::{
    text::{self, TextMask},
    window::{Layer, LayerId, LayerKind},
};
use crate::render::{
    Canvas, Sprite,
    scene::{self, Mask, PixelRect},
};

const ERROR_INVALID_PARAMETER: u32 = 87;

struct Card {
    layer: Layer,
    /// Готовая панель, premultiplied.
    pixels: Vec<u32>,
    size: (u32, u32),
    /// Масштаб, с которым панель нарисована.
    ui: f32,
    /// Окно, высота панели в нём и прозрачность последнего показа.
    drawn: Option<(PixelRect, u32, u8)>,
}

fn mask(text: &TextMask) -> Option<Mask<'_>> {
    (text.width > 0 && text.height > 0).then_some(Mask {
        coverage: &text.coverage,
        width: text.width,
        height: text.height,
    })
}

impl Card {
    fn new(notification: &Notification, ui: f32) -> Result<Self, AgentError> {
        let width = scene::text_width(ui);
        let title = text::rasterize(
            &notification.title,
            (scene::TITLE_PX * ui).round() as u32,
            width,
            scene::title_limit(ui),
            true,
        )?;
        let room = scene::body_limit(title.height, ui);
        let body = match &notification.body {
            Some(body) => text::rasterize(
                body,
                (scene::BODY_PX * ui).round() as u32,
                width,
                room,
                false,
            )?,
            None => TextMask {
                width: 0,
                height: 0,
                coverage: Vec::new(),
            },
        };
        let (title, body) = (mask(&title), mask(&body));
        let size = scene::card_size(title.map(|m| m.height), body.map(|m| m.height), ui);
        let mut pixels = vec![0; size.0 as usize * size.1 as usize];
        let mut canvas = Canvas::new(&mut pixels, size.0 as usize, size.0, size.1).ok_or(
            AgentError::Platform {
                call: "Canvas::new",
                code: ERROR_INVALID_PARAMETER,
            },
        )?;
        scene::draw_card(&mut canvas, notification.tone, title, body, ui);
        Ok(Self {
            layer: Layer::new(LayerKind::Interactive)?,
            pixels,
            size,
            ui,
            drawn: None,
        })
    }

    fn draw(&mut self, item: &Item) -> Result<(), AgentError> {
        let place = scene::place_card(item.pos(), self.size, item.scale(), item.toss(), self.ui);
        let rect = place.window;
        let opacity = (item.opacity().clamp(0.0, 1.0) * 255.0).round() as u8;
        if item.scale() <= 0.0 || rect.width == 0 || rect.height == 0 || opacity == 0 {
            self.hide();
            return Ok(());
        }
        let drawn = (rect, place.panel.y.to_bits(), opacity);
        if self.drawn == Some(drawn) {
            return Ok(());
        }
        let mut canvas = self.layer.canvas(rect.width, rect.height)?;
        scene::draw_card_shadow(&mut canvas, &place, self.ui);
        let panel = Sprite {
            pixels: &self.pixels,
            stride: self.size.0 as usize,
            rect: [0, 0, self.size.0, self.size.1],
        };
        canvas.blit(panel, place.panel, item.scale(), false, 1.0);
        self.layer
            .present(rect.x, rect.y, rect.width, rect.height, opacity)?;
        if !self.layer.is_visible() {
            self.layer.show();
        }
        self.drawn = Some(drawn);
        Ok(())
    }

    fn hide(&mut self) {
        self.drawn = None;
        if self.layer.is_visible() {
            self.layer.hide();
        }
    }
}

#[derive(Default)]
pub(super) struct Cards {
    cards: HashMap<ItemId, Card>,
}

impl Cards {
    pub fn item_at(&self, layer: LayerId) -> Option<ItemId> {
        self.cards
            .iter()
            .find(|(_, card)| card.layer.id() == layer)
            .map(|(id, _)| *id)
    }

    pub fn hide(&mut self) {
        self.cards.values_mut().for_each(Card::hide);
    }

    /// Окна появляются и исчезают вместе с карточками модели.
    pub fn sync(&mut self, items: &Items, ui: f32) {
        self.cards.retain(|id, _| items.get(*id).is_some());
        for item in items.iter() {
            let card = match self.cards.entry(item.id()) {
                Entry::Occupied(card) => card.into_mut(),
                Entry::Vacant(slot) => match Card::new(item.notification(), ui) {
                    Ok(card) => slot.insert(card),
                    // Не получилось сейчас — попробуем в следующем кадре
                    Err(_) => continue,
                },
            };
            if card.draw(item).is_err() {
                card.drawn = None;
            }
        }
    }
}
