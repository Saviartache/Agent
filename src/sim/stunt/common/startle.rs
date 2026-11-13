//! Испуг: курсор резко пролетел рядом — подпрыгивает, круглые глаза, отскакивает
//! от курсора. Без курсора (по команде) — прыжок на месте.

use std::f32::consts::PI;

use super::super::Progress;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::rig::Eyes;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Логические px, время — с.
const HOP: f32 = 18.0;
const HOP_SECS: f32 = 0.35;
const FLING_SPEED: f32 = 260.0;
const FRICTION: f32 = 5.0;
const SECS: f32 = 1.2;
/// Круглые глаза, пока не успокоится.
const WIDE_SECS: f32 = 0.9;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Startle {
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_startle(&mut self) -> Option<Startle> {
        let away = self
            .cursor
            .map_or(Vec2::ZERO, |cursor| (self.body.pos - cursor).normalized());
        self.halt();
        self.drive = Some(Drive::new(away * (FLING_SPEED * self.ui)).friction(FRICTION));
        self.burst(Burst::Dust, self.body.pos);
        Some(Startle { t: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_startle(
        &mut self,
        mut act: Startle,
        dt: f32,
    ) -> Progress<Startle> {
        act.t += dt;
        if act.t >= SECS {
            self.halt();
            return Progress::Finished;
        }
        self.antic.lift = HOP * (PI * act.t / HOP_SECS).sin().max(0.0);
        if act.t < WIDE_SECS {
            self.antic.eyes = Eyes::Wide;
        }
        Progress::Going(act)
    }
}
