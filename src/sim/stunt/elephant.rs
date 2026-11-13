//! Сценки слоника и всё, что у слоника своё.

pub(super) mod mouse_scare;
pub(super) mod shower;
pub(super) mod trumpet;
pub(super) mod trunk_nap;

use crate::behavior::Facing;
use crate::math::Vec2;
use crate::sim::Sim;

pub(super) const NOTES: &[&str] = &[
    "Ту-ру-ру!",
    "Я всё запомнил.",
    "Хобот — это нос. И рука. И душ.",
    "Где тут мышь? Я не боюсь. Почти.",
    "Слон в посудной лавке — это навет.",
];

/// Кончик хобота у земли от точки под телом, design px (`trunk_down(1.0)`).
const TRUNK_REACH: f32 = 32.0;

impl Sim {
    /// Хобот к земле, `depth` 0…1; при 1 кончик ложится в `TRUNK_REACH` перед телом.
    fn trunk_down(&mut self, depth: f32) {
        self.antic.neck_forward = 12.0 * depth;
        self.antic.neck_up = -15.0 * depth;
        self.antic.head_pitch = -0.6 * depth;
    }

    /// Встаёт боком к зрителю: поднятый хобот виден целиком.
    fn face_sideways(&mut self) {
        let x = match self.facing {
            Facing::Left => -1.0,
            Facing::Right => 1.0,
        };
        self.body.heading = Vec2::new(x, 0.0);
    }
}
