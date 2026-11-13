//! Куча листьев: появляется поодаль, гусь замечает её, разбегается, пробегает насквозь,
//! листья разлетаются; в конце он радуется.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const PILE_RADIUS: f32 = 30.0;
const PILE_LEAVES: usize = 60;
/// Нетронутая куча тает сама.
const PILE_TTL: f32 = 16.0;
/// Куча появляется на таком расстоянии: есть где разбежаться.
const PILE_DISTANCE: [f32; 2] = [280.0, 520.0];
const PILE_MIN_DISTANCE: f32 = 120.0;
/// Гусь замечает кучу и только потом бежит.
const NOTICE_SECS: f32 = 0.7;
/// Бежит сквозь кучу и дальше, а не тормозит перед ней.
const RUN_THROUGH: f32 = 140.0;
/// Не добежал за это время — сценка сорвалась.
const CHASE_SECS: f32 = 12.0;
/// После удара добегает и радуется.
const CHEER_SECS: f32 = 1.1;
/// Удар задевает листья чуть шире самой кучи.
const KICK_REACH: f32 = 1.3;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct LeafPile {
    pile: Vec2,
    /// До старта разбега.
    notice: f32,
    /// До срыва, а после удара — до радости.
    left: f32,
    kicked: bool,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_leaf_pile(&mut self) -> Option<LeafPile> {
        // С запасом на пробег за кучу: у края цель съехала бы вбок, и гусь пробежал бы мимо
        let pile = self.spot_away(PILE_DISTANCE, PILE_MIN_DISTANCE, PILE_RADIUS + RUN_THROUGH)?;
        let (radius, ui) = (PILE_RADIUS * self.ui, self.ui);
        self.leaves
            .heap(pile, radius, PILE_LEAVES, PILE_TTL, ui, &mut self.rng);
        Some(LeafPile {
            pile,
            notice: NOTICE_SECS,
            left: CHASE_SECS,
            kicked: false,
        })
    }

    pub(in crate::sim::stunt) fn step_leaf_pile(
        &mut self,
        mut act: LeafPile,
        dt: f32,
    ) -> Progress<LeafPile> {
        if act.notice > 0.0 {
            act.notice -= dt;
            if act.notice <= 0.0 {
                let beyond = (act.pile - self.body.pos).normalized() * (RUN_THROUGH * self.ui);
                let target = self.bounds.clamp(act.pile + beyond, self.edge_margin());
                self.brain.push(Directive::RunTo(target));
            }
        }
        if !act.kicked && self.body.pos.distance(act.pile) <= PILE_RADIUS * self.ui {
            let reach = PILE_RADIUS * KICK_REACH * self.ui;
            self.leaves
                .kick(act.pile, reach, self.body.vel, self.ui, &mut self.rng);
            self.burst(Burst::Dust, act.pile);
            act.kicked = true;
            act.left = CHEER_SECS;
        }
        act.left -= dt;
        if act.left > 0.0 {
            return Progress::Going(act);
        }
        if !act.kicked {
            return Progress::Dropped;
        }
        self.perform_quietly(self.character.actions().celebrate);
        Progress::Finished
    }
}
