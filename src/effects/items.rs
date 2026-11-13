//! Карточки, которые персонаж приносит на экран, у каждой своя жизнь.

use super::sane_dt;
use crate::animation::tween::Easing;
use crate::command::Notification;
use crate::math::Vec2;

/// Дольше карточку не носят: кладут где есть.
const CARRY_MAX: f32 = 5.0;
const SETTLE: f32 = 0.5;
const LEAVE: f32 = 0.3;
const FADE_IN: f32 = 0.2;
const POP: f32 = 0.3;
/// Скорость догоняния носильщика, 1/с.
const FOLLOW: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemPhase {
    Carried,
    Settling,
    Placed,
    Leaving,
}

#[derive(Clone, Debug)]
pub struct Item {
    id: ItemId,
    notification: Notification,
    pos: Vec2,
    phase: ItemPhase,
    /// С момента появления.
    age: f32,
    /// В текущей фазе.
    phase_time: f32,
    /// Начало и конец полёта в `Settling`.
    from: Vec2,
    rest: Vec2,
    by_user: bool,
}

impl Item {
    pub fn id(&self) -> ItemId {
        self.id
    }

    pub fn notification(&self) -> &Notification {
        &self.notification
    }

    /// Нижний центр, мировые пиксели.
    pub fn pos(&self) -> Vec2 {
        self.pos
    }

    pub fn opacity(&self) -> f32 {
        let appear = (self.age / FADE_IN).min(1.0);
        match self.phase {
            ItemPhase::Leaving => appear * (1.0 - self.phase_time / LEAVE).max(0.0),
            _ => appear,
        }
    }

    pub fn scale(&self) -> f32 {
        Easing::OutBack.apply(self.age / POP)
    }

    /// Высота броска, 0..1: карточку подбрасывают, и она падает на своё место.
    /// `pos` при этом — точка на земле под ней.
    pub fn toss(&self) -> f32 {
        match self.phase {
            ItemPhase::Settling => (self.phase_time / SETTLE * std::f32::consts::PI).sin(),
            _ => 0.0,
        }
    }

    pub fn phase(&self) -> ItemPhase {
        self.phase
    }

    fn begin_settle(&mut self, at: Vec2) {
        self.phase = ItemPhase::Settling;
        self.phase_time = 0.0;
        self.from = self.pos;
        self.rest = at;
    }

    fn begin_leave(&mut self, by_user: bool) {
        self.phase = ItemPhase::Leaving;
        self.phase_time = 0.0;
        self.by_user = by_user;
    }

    /// Переносит остаток шага через границы фаз; `true` — карточка исчезла.
    fn advance(&mut self, mut dt: f32, carrier: Vec2) -> bool {
        self.age += dt;
        loop {
            let elapsed = self.phase_time + dt;
            match self.phase {
                ItemPhase::Carried => {
                    self.pos = self.pos.lerp(carrier, 1.0 - (-FOLLOW * dt).exp());
                    if elapsed < CARRY_MAX {
                        self.phase_time = elapsed;
                        return false;
                    }
                    dt = elapsed - CARRY_MAX;
                    self.begin_settle(self.pos);
                }
                ItemPhase::Settling => {
                    if elapsed < SETTLE {
                        self.phase_time = elapsed;
                        self.pos = self
                            .from
                            .lerp(self.rest, Easing::OutQuad.apply(elapsed / SETTLE));
                        return false;
                    }
                    dt = elapsed - SETTLE;
                    self.pos = self.rest;
                    self.phase = ItemPhase::Placed;
                    self.phase_time = 0.0;
                }
                ItemPhase::Placed => {
                    let ttl = self.notification.ttl.as_secs_f32();
                    if elapsed < ttl {
                        self.phase_time = elapsed;
                        return false;
                    }
                    dt = elapsed - ttl;
                    self.begin_leave(false);
                }
                ItemPhase::Leaving => {
                    self.phase_time = elapsed.min(LEAVE);
                    return elapsed >= LEAVE;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed {
    pub id: ItemId,
    pub by_user: bool,
}

#[derive(Clone, Debug)]
pub struct Items {
    items: Vec<Item>,
    max: usize,
    next_id: u32,
    /// Вытесненные переполнением; сообщаются в следующем `update`.
    evicted: Vec<ItemId>,
}

impl Items {
    pub fn new(max: usize) -> Self {
        let max = max.max(1);
        Self {
            items: Vec::with_capacity(max),
            max,
            next_id: 0,
            evicted: Vec::new(),
        }
    }

    pub fn spawn(&mut self, notification: Notification, carrier: Vec2) -> ItemId {
        if self.items.len() == self.max {
            self.evicted.push(self.items.remove(0).id);
        }
        let id = ItemId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.items.push(Item {
            id,
            notification,
            pos: carrier,
            phase: ItemPhase::Carried,
            age: 0.0,
            phase_time: 0.0,
            from: carrier,
            rest: carrier,
            by_user: false,
        });
        id
    }

    pub fn place(&mut self, id: ItemId, at: Vec2) -> bool {
        match self.find_mut(id) {
            Some(item) if item.phase == ItemPhase::Carried => {
                item.begin_settle(at);
                true
            }
            _ => false,
        }
    }

    pub fn dismiss(&mut self, id: ItemId, by_user: bool) -> bool {
        match self.find_mut(id) {
            Some(item) if item.phase != ItemPhase::Leaving => {
                item.begin_leave(by_user);
                true
            }
            _ => false,
        }
    }

    pub fn update(&mut self, dt: f32, carrier: Vec2, closed: &mut Vec<Closed>) {
        closed.extend(
            self.evicted
                .drain(..)
                .map(|id| Closed { id, by_user: false }),
        );
        let dt = sane_dt(dt);
        self.items.retain_mut(|item| {
            let gone = item.advance(dt, carrier);
            if gone {
                closed.push(Closed {
                    id: item.id,
                    by_user: item.by_user,
                });
            }
            !gone
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> + '_ {
        self.items.iter()
    }

    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn relocate(&mut self, clamp: impl Fn(Vec2) -> Vec2) {
        for item in &mut self.items {
            match item.phase {
                ItemPhase::Settling => item.rest = clamp(item.rest),
                ItemPhase::Placed => {
                    item.rest = clamp(item.rest);
                    item.pos = item.rest;
                }
                ItemPhase::Carried | ItemPhase::Leaving => {}
            }
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.evicted.clear();
    }

    fn find_mut(&mut self, id: ItemId) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.id == id)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const HOME: Vec2 = Vec2::new(100.0, 100.0);

    fn note(ttl: f32) -> Notification {
        let mut note = Notification::info("тест");
        note.ttl = Duration::from_secs_f32(ttl);
        note
    }

    fn tick(items: &mut Items, dt: f32) -> Vec<Closed> {
        let mut closed = Vec::new();
        items.update(dt, HOME, &mut closed);
        closed
    }

    fn phase(items: &Items, id: ItemId) -> Option<ItemPhase> {
        items.get(id).map(Item::phase)
    }

    #[test]
    fn full_life_cycle_ends_by_ttl() {
        let mut items = Items::new(4);
        let id = items.spawn(note(1.0), HOME);
        assert_eq!(phase(&items, id), Some(ItemPhase::Carried));
        assert!(items.place(id, Vec2::new(-50.0, 20.0)));
        assert_eq!(phase(&items, id), Some(ItemPhase::Settling));
        assert!(tick(&mut items, 0.2).is_empty());
        let mid = items.get(id).unwrap().pos();
        assert!(mid.x < 100.0 && mid.x > -50.0);
        // В полёте карточка подброшена, на месте — лежит
        assert!(items.get(id).unwrap().toss() > 0.5);
        tick(&mut items, 0.35);
        assert_eq!(phase(&items, id), Some(ItemPhase::Placed));
        assert_eq!(items.get(id).unwrap().pos(), Vec2::new(-50.0, 20.0));
        assert_eq!(items.get(id).unwrap().toss(), 0.0);
        tick(&mut items, 0.9);
        assert_eq!(phase(&items, id), Some(ItemPhase::Placed));
        tick(&mut items, 0.2);
        assert_eq!(phase(&items, id), Some(ItemPhase::Leaving));
        assert!(items.get(id).unwrap().opacity() < 1.0);
        assert_eq!(tick(&mut items, 0.5), [Closed { id, by_user: false }]);
        assert!(items.is_empty());
    }

    #[test]
    fn pop_in_and_fade_in_on_spawn() {
        let mut items = Items::new(1);
        let id = items.spawn(note(5.0), HOME);
        let item = items.get(id).unwrap();
        assert_eq!((item.scale(), item.opacity()), (0.0, 0.0));
        tick(&mut items, 1.0);
        let item = items.get(id).unwrap();
        assert_eq!((item.scale(), item.opacity()), (1.0, 1.0));
    }

    #[test]
    fn user_dismiss_works_in_every_phase() {
        for stage in 0..3 {
            let mut items = Items::new(4);
            let id = items.spawn(note(5.0), HOME);
            if stage >= 1 {
                items.place(id, HOME);
            }
            if stage >= 2 {
                tick(&mut items, 0.5);
            }
            assert!(items.dismiss(id, true));
            assert!(!items.dismiss(id, true));
            assert_eq!(
                tick(&mut items, 1.0),
                [Closed { id, by_user: true }],
                "stage {stage}"
            );
        }
        assert!(!Items::new(1).dismiss(ItemId(7), true));
    }

    #[test]
    fn overflow_evicts_oldest_and_reports_it() {
        let mut items = Items::new(2);
        let a = items.spawn(note(5.0), HOME);
        let b = items.spawn(note(5.0), HOME);
        let c = items.spawn(note(5.0), HOME);
        assert_eq!(items.len(), 2);
        assert!(items.get(a).is_none());
        assert_eq!(
            tick(&mut items, 0.0),
            [Closed {
                id: a,
                by_user: false
            }]
        );
        assert!(tick(&mut items, 0.0).is_empty());
        let ids: Vec<_> = items.iter().map(Item::id).collect();
        assert_eq!(ids, [b, c]);
    }

    #[test]
    fn carried_item_follows_with_lag_then_times_out() {
        let mut items = Items::new(1);
        let id = items.spawn(note(1.0), Vec2::ZERO);
        tick(&mut items, 0.05);
        let pos = items.get(id).unwrap().pos();
        assert!(pos.x > 0.0 && pos.x < HOME.x);
        tick(&mut items, 5.0);
        assert_ne!(phase(&items, id), Some(ItemPhase::Carried));
        tick(&mut items, 1.0);
        assert_eq!(phase(&items, id), Some(ItemPhase::Placed));
        assert!(items.get(id).unwrap().pos().distance(HOME) < 1.0);
    }

    #[test]
    fn place_only_from_carried() {
        let mut items = Items::new(2);
        let id = items.spawn(note(5.0), HOME);
        assert!(items.place(id, HOME));
        assert!(!items.place(id, Vec2::ZERO));
        tick(&mut items, 1.0);
        assert!(!items.place(id, Vec2::ZERO));
        items.dismiss(id, false);
        assert!(!items.place(id, Vec2::ZERO));
        assert!(!items.place(ItemId(99), Vec2::ZERO));
    }

    #[test]
    fn relocate_moves_resting_points() {
        let mut items = Items::new(4);
        let placed = items.spawn(note(5.0), HOME);
        let settling = items.spawn(note(5.0), HOME);
        let carried = items.spawn(note(5.0), HOME);
        items.place(placed, Vec2::new(3000.0, 10.0));
        tick(&mut items, 1.0);
        items.place(settling, Vec2::new(3000.0, 20.0));
        items.relocate(|p| Vec2::new(p.x.min(1000.0), p.y));
        assert_eq!(items.get(placed).unwrap().pos(), Vec2::new(1000.0, 10.0));
        tick(&mut items, 1.0);
        assert_eq!(items.get(settling).unwrap().pos(), Vec2::new(1000.0, 20.0));
        assert_eq!(phase(&items, carried), Some(ItemPhase::Carried));
    }

    #[test]
    fn ids_are_unique() {
        let mut items = Items::new(2);
        let ids: Vec<_> = (0..10).map(|_| items.spawn(note(1.0), HOME)).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn empty_update_and_clear_are_quiet() {
        let mut items = Items::new(0);
        assert!(tick(&mut items, 1.0).is_empty());
        items.spawn(note(1.0), HOME);
        items.spawn(note(1.0), HOME);
        items.clear();
        assert!(items.is_empty());
        assert!(tick(&mut items, 1.0).is_empty());
    }

    #[test]
    fn huge_dt_closes_placed_item_exactly_once() {
        let mut items = Items::new(2);
        let id = items.spawn(note(1.0), HOME);
        items.place(id, HOME);
        tick(&mut items, 0.5);
        assert_eq!(tick(&mut items, 1e9), [Closed { id, by_user: false }]);
        assert!(tick(&mut items, 1e9).is_empty());
        let carried = items.spawn(note(1.0), HOME);
        assert_eq!(
            tick(&mut items, f32::INFINITY),
            [Closed {
                id: carried,
                by_user: false
            }]
        );
    }

    #[test]
    fn bad_dt_changes_nothing() {
        let mut items = Items::new(1);
        let id = items.spawn(note(1.0), Vec2::ZERO);
        tick(&mut items, f32::NAN);
        tick(&mut items, -1.0);
        let item = items.get(id).unwrap();
        assert_eq!((item.pos(), item.phase()), (Vec2::ZERO, ItemPhase::Carried));
    }
}
