//! Сценки пингвина и всё, что у пингвина своё.

pub(super) mod applause;
pub(super) mod strut;
pub(super) mod toboggan;
pub(super) mod wipeout;

/// Тело торчком ложится ничком: наклон, гасящий стойку, рад, и присед до земли, design px.
const PRONE_PITCH: f32 = -1.45;
const PRONE_CROUCH: f32 = 5.0;

pub(super) const NOTES: &[&str] = &[
    "Я не толстый, это перья.",
    "Тут холодно? Мне нормально.",
    "Фрак не снимаю.",
    "Иду. Медленно, но важно.",
    "Лёд не нужен, живот скользит.",
];

impl crate::sim::Sim {
    /// Лежит на животе; `k` — доля, 0 — стоит.
    fn prone(&mut self, k: f32) {
        self.antic.pitch = PRONE_PITCH * k;
        self.antic.crouch = PRONE_CROUCH * k;
        self.antic.tuck = k;
    }
}
