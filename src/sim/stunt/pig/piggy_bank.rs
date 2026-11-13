//! Копилка: ловит спиной падающие монетки.

use std::f32::consts::PI;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px (design px — у тела), время — с.
const COINS: usize = 3;
const DROP_GAP: f32 = 0.6;
const DROP_HEIGHT: f32 = 220.0;
const DROP_SPREAD: [f32; 2] = [70.0, 40.0];
const COIN_TTL: f32 = 30.0;
/// Высота спины, design px.
const BACK: f32 = 30.0;
const CATCH: f32 = 14.0;
/// Пропущенная монета лежит столько секунд.
const MISSED_LIES: f32 = 5.0;
const HOP_SECS: f32 = 0.25;
const HOP: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
enum Slot {
    /// Упадёт через столько секунд.
    Waiting(f32),
    Falling(PropId),
    Done,
}

#[derive(Debug)]
pub(in crate::sim) struct PiggyBank {
    slots: [Slot; COINS],
    hop: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_piggy_bank(&mut self) -> Option<PiggyBank> {
        Some(PiggyBank {
            slots: std::array::from_fn(|i| Slot::Waiting(DROP_GAP * i as f32)),
            hop: 0.0,
        })
    }

    pub(in crate::sim::stunt) fn step_piggy_bank(
        &mut self,
        mut act: PiggyBank,
        dt: f32,
    ) -> Progress<PiggyBank> {
        for i in 0..COINS {
            act.slots[i] = match act.slots[i] {
                Slot::Waiting(left) if left > dt => Slot::Waiting(left - dt),
                Slot::Waiting(_) => Slot::Falling(self.drop_coin()),
                Slot::Falling(id) => self.settle_coin(&mut act.hop, id, i == COINS - 1),
                Slot::Done => Slot::Done,
            };
        }
        if act.hop <= 0.0 && act.slots.iter().all(|slot| matches!(slot, Slot::Done)) {
            self.halt();
            return Progress::Finished;
        }
        // Бежит под ту монету, что падает первой
        let falling = act.slots.iter().find_map(|slot| match slot {
            Slot::Falling(id) => self.props.get(*id).map(|coin| coin.pos),
            _ => None,
        });
        if let Some(under) = falling {
            self.brain.push(Directive::RunTo(under));
        }
        if act.hop > 0.0 {
            let u = 1.0 - act.hop / HOP_SECS;
            self.antic.lift = HOP * (PI * u).sin().max(0.0);
            act.hop -= dt;
        }
        Progress::Going(act)
    }

    fn drop_coin(&mut self) -> PropId {
        let aside = Vec2::new(
            self.rng.range(-DROP_SPREAD[0], DROP_SPREAD[0]),
            self.rng.range(-DROP_SPREAD[1], DROP_SPREAD[1]),
        ) * self.ui;
        let at = self
            .bounds
            .nearest(self.body.pos)
            .clamp(self.body.pos + aside, self.edge_margin());
        self.props
            .spawn(PropKind::Coin, at, DROP_HEIGHT * self.ui, self.ui, COIN_TTL)
    }

    /// Падающая монета: поймана на спине, упала мимо или ещё летит.
    fn settle_coin(&mut self, hop: &mut f32, id: PropId, last: bool) -> Slot {
        let Some(coin) = self.props.get(id).copied() else {
            return Slot::Done;
        };
        let back = BACK * self.sprite;
        let below = (coin.pos - self.body.pos).length() <= CATCH * self.ui;
        if coin.z > 0.0 && coin.z <= back && below {
            self.props.remove(id);
            self.burst(Burst::Sparks, self.body.pos - Vec2::new(0.0, back));
            if last {
                self.burst(Burst::Hearts, self.head());
            }
            *hop = HOP_SECS;
            return Slot::Done;
        }
        if coin.z > 0.0 {
            return Slot::Falling(id);
        }
        if let Some(coin) = self.props.get_mut(id) {
            coin.expire_in(MISSED_LIES);
        }
        Slot::Done
    }
}
