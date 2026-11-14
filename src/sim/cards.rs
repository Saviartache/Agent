//! Карточки уведомлений: персонаж несёт карточку над головой и кладёт рядом с собой.

use super::{Sim, push_event};
use crate::behavior::{Directive, Facing};
use crate::command::Notification;
use crate::effects::items::{ItemId, ItemPhase};
use crate::event::AgentEvent;
use crate::math::{Rect, Vec2};
use crate::render::scene::CARD_WIDTH;

/// Столько карточка в руках, прежде чем её кладут, с.
const CARRY_SECS: f32 = 0.9;
/// Высота места под карточку в стопке, логические px.
const SLOT_HEIGHT: f32 = 100.0;
/// Просвет между персонажем и карточкой, и между карточкой и краем экрана.
const SIDE_GAP: f32 = 12.0;
const SCREEN_GAP: f32 = 8.0;
/// Карточка несётся чуть выше головы.
const CARRY_LIFT: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Carry {
    id: ItemId,
    left: f32,
}

impl Sim {
    /// Щелчок пользователя по карточке.
    pub fn dismiss_item(&mut self, id: ItemId) {
        self.items.dismiss(id, true);
    }

    pub(super) fn notify(&mut self, notification: Notification) {
        let id = self.items.spawn(notification, self.carry_point());
        if !self.alive() {
            let spot = self.card_spot();
            self.items.place(id, spot);
            return;
        }
        let interact = self.character.actions().interact;
        // Это действие — только перенос, приложению о нём не сообщаем
        self.silent = Some(interact);
        self.brain.push(Directive::Perform(interact));
        self.carrying.push(Carry {
            id,
            left: CARRY_SECS,
        });
    }

    pub(super) fn carry_point(&self) -> Vec2 {
        self.head() - Vec2::new(0.0, CARRY_LIFT * self.ui)
    }

    /// Карточки кладутся по очереди: срок у всех один, первая готова первой.
    pub(super) fn advance_cards(&mut self, dt: f32) {
        let alive = self.alive();
        for carry in &mut self.carrying {
            carry.left -= dt;
        }
        while self
            .carrying
            .first()
            .is_some_and(|carry| carry.left <= 0.0 || !alive)
        {
            let carry = self.carrying.remove(0);
            let spot = self.card_spot();
            self.items.place(carry.id, spot);
        }
    }

    pub(super) fn collect_closed(&mut self, dt: f32) {
        let carrier = self.carry_point();
        self.items.update(dt, carrier, &mut self.closed);
        if self.closed.is_empty() {
            return;
        }
        for closed in self.closed.drain(..) {
            let event = AgentEvent::NotificationClosed {
                by_user: closed.by_user,
            };
            push_event(&mut self.events, event);
        }
        self.carrying
            .retain(|carry| self.items.get(carry.id).is_some());
    }

    /// Нижний центр места: рядом с персонажем в сторону взгляда, выше уже лежащих;
    /// без персонажа — в углу первого монитора.
    fn card_spot(&self) -> Vec2 {
        let ui = self.ui;
        let (base, area) = if self.is_present() {
            let side = match self.facing {
                Facing::Left => -1.0,
                Facing::Right => 1.0,
            };
            let body_half = self.character.half_width() * self.sprite;
            let reach = (CARD_WIDTH * 0.5 + SIDE_GAP) * ui + body_half;
            (
                self.body.pos + Vec2::new(side * reach, 0.0),
                self.bounds.nearest(self.body.pos),
            )
        } else {
            let area = self.bounds.areas()[0];
            (area.max, area)
        };
        let stacked = self
            .items
            .iter()
            .filter(|item| matches!(item.phase(), ItemPhase::Settling | ItemPhase::Placed));
        let at = base - Vec2::new(0.0, stacked.count() as f32 * SLOT_HEIGHT * ui);
        let half = Vec2::new((CARD_WIDTH * 0.5 + SCREEN_GAP) * ui, 0.0);
        let room = Rect::new(
            area.min + half + Vec2::new(0.0, SLOT_HEIGHT * ui),
            area.max - half - Vec2::new(0.0, SCREEN_GAP * ui),
        );
        room.clamp(at, 0.0)
    }
}
