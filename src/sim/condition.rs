//! Состояния после сценок: грязная, оглушён, раздута, сонный, грязный нос.
//! Каждое 0…1 и гаснет само; на теле их показывает `Look`, оглушённого — шатание
//! и звёздочки над головой. Грязь стирается шагами (`muddy_step`), остальное — временем.

use std::f32::consts::TAU;

use super::Sim;
use crate::effects::particles::Burst;
use crate::rig::Look;

/// За столько секунд состояние сходит с 1 до 0.
const DAZED_SECS: f32 = 6.0;
const BLOATED_SECS: f32 = 8.0;
const SLEEPY_SECS: f32 = 30.0;
const SNOUT_SECS: f32 = 15.0;
/// Оглушённый шатается и сыплет звёздочками.
const DAZED_WOBBLE: f32 = 2.5;
const DAZED_HZ: f32 = 1.3;
const STARS_GAP: f32 = 0.7;
/// Сонный прикрывает глаза.
const SLEEPY_LIDS: f32 = 0.55;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Conditions {
    /// Грязь после лужи.
    pub muddy: f32,
    pub dazed: f32,
    /// Раздута после еды.
    pub bloated: f32,
    pub sleepy: f32,
    /// Грязный пятачок или клюв.
    pub snout: f32,
    stars_in: f32,
}

fn fade(value: &mut f32, dt: f32, secs: f32) {
    *value = (*value - dt / secs).max(0.0);
}

impl Conditions {
    fn fade(&mut self, dt: f32) {
        fade(&mut self.dazed, dt, DAZED_SECS);
        fade(&mut self.bloated, dt, BLOATED_SECS);
        fade(&mut self.sleepy, dt, SLEEPY_SECS);
        fade(&mut self.snout, dt, SNOUT_SECS);
    }

    pub fn look(&self) -> Look {
        Look {
            mud: self.muddy,
            bloat: self.bloated,
            snout: self.snout,
        }
    }
}

impl Sim {
    /// После шага сценки: состояния гаснут и добавляют своё к накладке позы.
    pub(super) fn tend_conditions(&mut self, dt: f32) {
        self.cond.fade(dt);
        let sleepy = self.cond.sleepy * SLEEPY_LIDS;
        self.antic.lids = self.antic.lids.max(sleepy);
        if self.cond.dazed <= 0.0 {
            self.cond.stars_in = 0.0;
            return;
        }
        let sway = (TAU * DAZED_HZ * self.clock).sin();
        self.antic.wobble += DAZED_WOBBLE * self.cond.dazed * sway;
        self.cond.stars_in -= dt;
        if self.cond.stars_in <= 0.0 {
            self.cond.stars_in = STARS_GAP;
            self.burst(Burst::Stars, self.head());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_but_mud_wears_off_by_time() {
        let mut cond = Conditions {
            muddy: 1.0,
            dazed: 1.0,
            bloated: 1.0,
            sleepy: 1.0,
            snout: 1.0,
            stars_in: 0.0,
        };
        cond.fade(DAZED_SECS * 0.5);
        assert!((cond.dazed - 0.5).abs() < 1e-4);
        cond.fade(SLEEPY_SECS);
        let look = cond.look();
        assert_eq!(cond.dazed + cond.bloated + cond.sleepy, 0.0);
        assert_eq!((look.mud, look.snout), (1.0, 0.0));
    }
}
