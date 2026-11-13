//! Общие сценки: их играет любой персонаж, процедурный и спрайтовый.

pub(super) mod ball;
pub(super) mod boop;
pub(super) mod butterfly;
pub(super) mod chase;
pub(super) mod cursor_heist;
pub(super) mod dance;
pub(super) mod edge_tumble;
pub(super) mod fake_death;
pub(super) mod flatten;
pub(super) mod gift;
pub(super) mod hiccups;
pub(super) mod nap;
pub(super) mod note;
pub(super) mod peek;
pub(super) mod self_scare;
pub(super) mod slip;
pub(super) mod sneeze;
pub(super) mod stare;
pub(super) mod startle;
pub(super) mod stretch;
pub(super) mod trip_over_card;
pub(super) mod zoomies;

/// Шутки записки у спрайтового набора: вид неизвестен.
pub(super) const NOTES: &[&str] = &[
    "Привет!",
    "Я тут посижу.",
    "Не обращай внимания.",
    "Всё под контролем.",
];
