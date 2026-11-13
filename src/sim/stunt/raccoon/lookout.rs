//! Дозор: встаёт столбиком на задние лапы, озирается по сторонам и опускается обратно.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::animation::tween::Easing;
use crate::sim::Sim;

/// Время — с; углы — рад.
const RISE_SECS: f32 = 0.5;
const LOOK_SECS: f32 = 3.0;
const DROP_SECS: f32 = 0.4;
/// Корпус почти вертикально: зад на земле, грудь вверх.
const UPRIGHT: f32 = 1.15;
const LOOKS: f32 = 2.0;
const LOOK_ANGLE: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Lookout {
    t: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_lookout(&mut self) -> Option<Lookout> {
        self.halt();
        Some(Lookout { t: 0.0 })
    }

    pub(in crate::sim::stunt) fn step_lookout(
        &mut self,
        act: Lookout,
        dt: f32,
    ) -> Progress<Lookout> {
        let t = act.t + dt;
        let end = RISE_SECS + LOOK_SECS + DROP_SECS;
        if t >= end {
            return Progress::Finished;
        }
        let up = if t < RISE_SECS {
            Easing::OutBack.apply(t / RISE_SECS)
        } else {
            1.0 - Easing::InQuad.apply(((t - RISE_SECS - LOOK_SECS) / DROP_SECS).max(0.0))
        };
        self.antic.pitch = UPRIGHT * up;
        self.antic.neck_up = 4.0 * up;
        let looking = ((t - RISE_SECS) / LOOK_SECS).clamp(0.0, 1.0);
        self.antic.head_yaw = LOOK_ANGLE * (TAU * LOOKS * looking).sin() * up;
        Progress::Going(Lookout { t })
    }
}
