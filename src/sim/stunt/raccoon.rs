//! Сценки енота и всё, что у енота своё.

pub(super) mod lookout;
pub(super) mod rinse;
pub(super) mod rummage;
pub(super) mod stash;

use crate::effects::props::PropId;
use crate::math::Vec2;
use crate::sim::Sim;

pub(super) const NOTES: &[&str] = &[
    "Это не я.",
    "Ничего не брал.",
    "Блестящее — моё.",
    "Сначала помыть, потом съесть.",
    "Мусорка — это шведский стол.",
];

/// Мордочка у земли перед передними лапами от точки под телом, design px (`paws_down(1.0)`).
const PAWS_REACH: f32 = 20.0;

impl Sim {
    /// Морда вниз между передними лапами, `depth` 0…1: лапы что-то держат у земли.
    fn paws_down(&mut self, depth: f32) {
        self.antic.pitch = -0.25 * depth;
        self.antic.crouch = 3.0 * depth;
        self.antic.neck_forward = 10.0 * depth;
        self.antic.neck_up = -14.0 * depth;
        self.antic.head_pitch = -0.9 * depth;
    }

    /// Предмет в пасти: середина у кончика мордочки.
    fn hold_in_muzzle(&mut self, id: PropId) {
        let (at, z) = (self.muzzle_ground(), self.muzzle_height());
        if let Some(prop) = self.props.get_mut(id) {
            prop.pos = at;
            prop.z = (z - prop.radius * 0.7).max(0.0);
            prop.vel = Vec2::ZERO;
            prop.vz = 0.0;
        }
    }
}
