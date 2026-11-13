//! Рытьё: роет рылом, летят комья, на третий раз — жёлудь, который свинка хрустит.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с, частоты — Гц.
const SPOT_DISTANCE: [f32; 2] = [100.0, 200.0];
const SPOT_MIN: f32 = 80.0;
const ARRIVE: f32 = 14.0;
/// Не дошла за это время — сценка сорвалась.
const WALK_SECS: f32 = 8.0;
const DIGS: u8 = 3;
const DIG_SECS: f32 = 1.0;
const DIG_HZ: f32 = 3.0;
const LIFT_SECS: f32 = 0.3;
const DIRT_GAP: f32 = 0.25;
const PIT_RADIUS: f32 = 8.0;
const PIT_TTL: f32 = 10.0;
const ACORN_VZ: f32 = 200.0;
const ACORN_TTL: f32 = 6.0;
/// Жёлудь исчезает на этой секунде хруста.
const SWALLOW_AT: f32 = 0.6;
const MUNCH_SECS: f32 = 1.5;
const MUNCH_HZ: f32 = 4.0;
const SNOUT: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk(f32),
    Dig(f32),
    Lift(f32),
    Munch(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Dig {
    spot: Vec2,
    phase: Phase,
    dug: u8,
    dirt_in: f32,
    acorn: Option<PropId>,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_dig(&mut self) -> Option<Dig> {
        let spot = self.spot_away(SPOT_DISTANCE, SPOT_MIN, ARRIVE)?;
        self.brain.push(Directive::MoveTo(spot));
        Some(Dig {
            spot,
            phase: Phase::Walk(WALK_SECS),
            dug: 0,
            dirt_in: 0.0,
            acorn: None,
        })
    }

    pub(in crate::sim::stunt) fn step_dig(&mut self, mut act: Dig, dt: f32) -> Progress<Dig> {
        act.phase = match act.phase {
            Phase::Walk(_) if self.body.pos.distance(act.spot) <= ARRIVE * self.ui => {
                self.halt();
                Phase::Dig(DIG_SECS)
            }
            Phase::Walk(left) if left > dt => Phase::Walk(left - dt),
            Phase::Walk(_) => return Progress::Dropped,
            Phase::Dig(left) => self.dig(&mut act, left, dt),
            Phase::Lift(left) if left > dt => Phase::Lift(left - dt),
            Phase::Lift(_) => Phase::Dig(DIG_SECS),
            Phase::Munch(left) => match self.munch(&mut act, left, dt) {
                Some(phase) => phase,
                None => return Progress::Finished,
            },
        };
        Progress::Going(act)
    }

    fn dig(&mut self, act: &mut Dig, left: f32, dt: f32) -> Phase {
        let spent = DIG_SECS - left;
        self.antic.pitch = -0.6;
        self.antic.neck_up = -6.0;
        self.antic.head_pitch = -0.9;
        self.antic.neck_forward = 5.0 + 1.5 * (TAU * DIG_HZ * spent).sin();
        self.cond.snout = SNOUT;
        act.dirt_in -= dt;
        if act.dirt_in <= 0.0 {
            act.dirt_in = DIRT_GAP;
            self.burst(Burst::Dirt, self.head());
        }
        if left > dt {
            return Phase::Dig(left - dt);
        }
        act.dug += 1;
        let pit = self.head();
        self.mud
            .pour(pit, PIT_RADIUS * self.ui, PIT_TTL, &mut self.rng);
        if act.dug < DIGS {
            return Phase::Lift(LIFT_SECS);
        }
        let id = self
            .props
            .spawn(PropKind::Acorn, pit, 0.0, self.ui, ACORN_TTL);
        if let Some(acorn) = self.props.get_mut(id) {
            acorn.vz = ACORN_VZ * self.ui;
        }
        act.acorn = Some(id);
        Phase::Munch(MUNCH_SECS)
    }

    /// `None` — дожевала.
    fn munch(&mut self, act: &mut Dig, left: f32, dt: f32) -> Option<Phase> {
        let spent = MUNCH_SECS - left;
        if spent >= SWALLOW_AT
            && let Some(id) = act.acorn.take()
        {
            self.props.remove(id);
        }
        self.antic.mouth = 0.4 + 0.4 * (TAU * MUNCH_HZ * spent).sin();
        self.antic.head_pitch = -0.3;
        (left > dt).then_some(Phase::Munch(left - dt))
    }
}
