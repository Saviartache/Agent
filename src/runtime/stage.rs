//! Окна персонажа и его эффектов; перерисовывает только то, что изменилось.

use crate::animation::SheetId;
use crate::error::AgentError;
use crate::math::{Rect, Vec2};
use crate::platform::window::{Layer, LayerId, LayerKind};
use crate::render::creature;
use crate::render::scene::{self, CharacterView, PixelRect};
use crate::rig::Pose;
use crate::sim::Sim;

/// Всё, от чего зависит картинка персонажа.
#[derive(Clone, Copy, PartialEq)]
enum Drawn {
    Sprite {
        sheet: SheetId,
        frame: [u32; 4],
        rect: PixelRect,
        flip: bool,
        scale_bits: u32,
    },
    /// Холст процедурного тела; сама поза лежит в `Stage::pose`.
    Creature { rect: PixelRect },
}

pub(super) struct Stage {
    character: Layer,
    effects: Layer,
    drawn: Option<Drawn>,
    /// Поза, которой нарисован `Drawn::Creature`.
    pose: Pose,
}

impl Stage {
    pub fn new() -> Result<Self, AgentError> {
        let character = Layer::new(LayerKind::Interactive)?;
        let effects = Layer::new(LayerKind::ClickThrough)?;
        effects.stack_below(&character);
        Ok(Self {
            character,
            effects,
            drawn: None,
            pose: Pose::default(),
        })
    }

    pub fn character_id(&self) -> LayerId {
        self.character.id()
    }

    pub fn popup_menu(&self, at: Vec2, items: &[(u32, &str)]) -> Option<u32> {
        self.character.popup_menu(at, items)
    }

    pub fn hide(&mut self) {
        self.drawn = None;
        self.hide_character();
        self.hide_effects();
    }

    pub fn draw(&mut self, sim: &Sim) {
        // Сбой кадра не фатален: слой пропускается, следующий кадр перерисует
        if self.draw_character(sim).is_err() {
            self.drawn = None;
        }
        if self.draw_effects(sim).is_err() {
            self.hide_effects();
        }
    }

    fn draw_character(&mut self, sim: &Sim) -> Result<(), AgentError> {
        let Some(view) = sim.character() else {
            self.drawn = None;
            self.hide_character();
            return Ok(());
        };
        let (rect, drawn) = match &view {
            CharacterView::Sprite(sprite) => {
                let rect = scene::character_rect(sprite);
                let drawn = Drawn::Sprite {
                    sheet: sprite.sheet,
                    frame: sprite.sprite.rect,
                    rect,
                    flip: sprite.flip,
                    scale_bits: sprite.scale.to_bits(),
                };
                (rect, drawn)
            }
            CharacterView::Creature { pose, extent } => {
                let rect = creature::rect(pose, *extent);
                (rect, Drawn::Creature { rect })
            }
        };
        let same_pose = match &view {
            CharacterView::Creature { pose, .. } => self.pose == **pose,
            CharacterView::Sprite(_) => true,
        };
        if self.drawn == Some(drawn) && same_pose {
            return Ok(());
        }
        let mut canvas = self.character.canvas(rect.width, rect.height)?;
        match &view {
            CharacterView::Sprite(sprite) => scene::draw_character(&mut canvas, sprite),
            CharacterView::Creature { pose, .. } => {
                let origin = Vec2::new(rect.x as f32, rect.y as f32);
                creature::draw(&mut canvas, origin, pose);
                if let Some(screen) = sim.character_clip() {
                    canvas.clear_outside(Rect::new(screen.min - origin, screen.max - origin));
                }
                self.pose.clone_from(pose);
            }
        }
        self.character
            .present(rect.x, rect.y, rect.width, rect.height, u8::MAX)?;
        if !self.character.is_visible() {
            self.character.show();
        }
        self.drawn = Some(drawn);
        Ok(())
    }

    fn draw_effects(&mut self, sim: &Sim) -> Result<(), AgentError> {
        let (effects, ui) = (sim.effects(), sim.ui_scale());
        let Some(area) = scene::effects_rect(&effects, ui) else {
            self.hide_effects();
            return Ok(());
        };
        let rect = scene::enclosing(area);
        let mut canvas = self.effects.canvas(rect.width, rect.height)?;
        let origin = Vec2::new(rect.x as f32, rect.y as f32);
        scene::draw_effects(&mut canvas, origin, &effects, ui);
        self.effects
            .present(rect.x, rect.y, rect.width, rect.height, u8::MAX)?;
        if !self.effects.is_visible() {
            self.effects.show();
            // `show` поднимает окно на самый верх, а следы должны лежать под персонажем
            self.effects.stack_below(&self.character);
        }
        Ok(())
    }

    fn hide_character(&mut self) {
        if self.character.is_visible() {
            self.character.hide();
        }
    }

    fn hide_effects(&mut self) {
        if self.effects.is_visible() {
            self.effects.hide();
        }
    }
}
