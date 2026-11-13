//! Сценки: персонаж сам разыгрывает что-то с предметами, курсором или запиской.
//! Общие лежат в `common/`, видовые — в каталоге своего тела (`goose/`, `pig/`, `penguin/`…);
//! чья сценка, решает `Stunt::cast`. Новая сценка — вариант `Stunt`, файл со своим
//! состоянием и методами `begin_*`/`step_*` и строка в `acts!` здесь.
//!
//! Шаг сценки каждый раз заново задаёт накладку позы (`Sim::antic`); толчок тела
//! (`Sim::drive`), гусята (`Sim::flock`) и накладка гаснут вместе со сценкой.

mod common;
mod dachshund;
mod elephant;
mod goose;
mod penguin;
mod pig;
mod raccoon;
mod reflex;

use std::f32::consts::TAU;

use super::{Sim, push_event};
use crate::action::{ActionId, names};
use crate::assets::Character;
use crate::behavior::{Directive, Intent};
use crate::command::Stunt;
use crate::event::AgentEvent;
use crate::math::Vec2;
use crate::rig::{Antic, RigKind};

/// Сколько раз ищем место для предмета или пробежки, прежде чем сдаться.
const SPOT_TRIES: usize = 8;

/// Действия, которые не годятся на роль «чего-нибудь случайного»: служебные и циклы.
const NOT_AMBIENT: [&str; 11] = [
    names::SPAWN,
    names::DESPAWN,
    names::IDLE,
    names::WALK,
    names::RUN,
    names::TURN,
    names::DRAGGED,
    names::WAKE_UP,
    names::SLEEP,
    names::BUSY,
    names::THINK,
];

/// Реестр: вариант `Stunt`, состояние и его `begin_*`/`step_*`.
macro_rules! acts {
    ($($stunt:ident: $group:ident::$module:ident::$state:ident => $begin:ident, $step:ident;)*) => {
        /// Идущая сценка.
        #[derive(Debug)]
        pub(super) enum Act {
            $($stunt($group::$module::$state),)*
        }

        impl Act {
            fn stunt(&self) -> Stunt {
                match self {
                    $(Self::$stunt(_) => Stunt::$stunt,)*
                }
            }
        }

        impl Sim {
            fn begin(&mut self, stunt: Stunt) -> Option<Act> {
                Some(match stunt {
                    $(Stunt::$stunt => Act::$stunt(self.$begin()?),)*
                })
            }

            fn step_act(&mut self, act: Act, dt: f32) -> Progress<Act> {
                match act {
                    $(Act::$stunt(state) => self.$step(state, dt).map(Act::$stunt),)*
                }
            }
        }
    };
}

acts! {
    CursorHeist: common::cursor_heist::CursorHeist => begin_cursor_heist, step_cursor_heist;
    Zoomies: common::zoomies::Zoomies => begin_zoomies, step_zoomies;
    Note: common::note::Note => begin_note, step_note;
    Chase: common::chase::Chase => begin_chase, step_chase;
    Boop: common::boop::Boop => begin_boop, step_boop;
    Startle: common::startle::Startle => begin_startle, step_startle;
    Sneeze: common::sneeze::Sneeze => begin_sneeze, step_sneeze;
    Hiccups: common::hiccups::Hiccups => begin_hiccups, step_hiccups;
    Dance: common::dance::Dance => begin_dance, step_dance;
    Stretch: common::stretch::Stretch => begin_stretch, step_stretch;
    Stare: common::stare::Stare => begin_stare, step_stare;
    Nap: common::nap::Nap => begin_nap, step_nap;
    Gift: common::gift::Gift => begin_gift, step_gift;
    Ball: common::ball::Ball => begin_ball, step_ball;
    Slip: common::slip::Slip => begin_slip, step_slip;
    TripOverCard: common::trip_over_card::TripOverCard => begin_trip_over_card, step_trip_over_card;
    SelfScare: common::self_scare::SelfScare => begin_self_scare, step_self_scare;
    Peek: common::peek::Peek => begin_peek, step_peek;
    EdgeTumble: common::edge_tumble::EdgeTumble => begin_edge_tumble, step_edge_tumble;
    Butterfly: common::butterfly::Butterfly => begin_butterfly, step_butterfly;
    Flatten: common::flatten::Flatten => begin_flatten, step_flatten;
    FakeDeath: common::fake_death::FakeDeath => begin_fake_death, step_fake_death;
    LeafPile: goose::leaf_pile::LeafPile => begin_leaf_pile, step_leaf_pile;
    WallSlam: goose::wall_slam::WallSlam => begin_wall_slam, step_wall_slam;
    Honk: goose::honk::Honk => begin_honk, step_honk;
    Hiss: goose::hiss::Hiss => begin_hiss, step_hiss;
    March: goose::march::March => begin_march, step_march;
    Skid: goose::skid::Skid => begin_skid, step_skid;
    Flight: goose::flight::Flight => begin_flight, step_flight;
    Preen: goose::preen::Preen => begin_preen, step_preen;
    Crumbs: goose::crumbs::Crumbs => begin_crumbs, step_crumbs;
    Bath: goose::bath::Bath => begin_bath, step_bath;
    Nest: goose::nest::Nest => begin_nest, step_nest;
    EggSurprise: goose::egg_surprise::EggSurprise => begin_egg_surprise, step_egg_surprise;
    Scribble: goose::scribble::Scribble => begin_scribble, step_scribble;
    NeckTangle: goose::neck_tangle::NeckTangle => begin_neck_tangle, step_neck_tangle;
    Goslings: goose::goslings::Goslings => begin_goslings, step_goslings;
    MudBath: pig::mud_bath::MudBath => begin_mud_bath, step_mud_bath;
    Oops: pig::oops::Oops => begin_oops, step_oops;
    FartJump: pig::fart_jump::FartJump => begin_fart_jump, step_fart_jump;
    Faceplant: pig::faceplant::Faceplant => begin_faceplant, step_faceplant;
    Slide: pig::slide::Slide => begin_slide, step_slide;
    Stuck: pig::stuck::Stuck => begin_stuck, step_stuck;
    ShakeOff: pig::shake_off::ShakeOff => begin_shake_off, step_shake_off;
    MudNap: pig::mud_nap::MudNap => begin_mud_nap, step_mud_nap;
    PuddleHop: pig::puddle_hop::PuddleHop => begin_puddle_hop, step_puddle_hop;
    Dig: pig::dig::Dig => begin_dig, step_dig;
    Sniff: pig::sniff::Sniff => begin_sniff, step_sniff;
    Snack: pig::snack::Snack => begin_snack, step_snack;
    Bulldozer: pig::bulldozer::Bulldozer => begin_bulldozer, step_bulldozer;
    TailChase: pig::tail_chase::TailChase => begin_tail_chase, step_tail_chase;
    PiggyBank: pig::piggy_bank::PiggyBank => begin_piggy_bank, step_piggy_bank;
    BellyUp: pig::belly_up::BellyUp => begin_belly_up, step_belly_up;
    Slipper: dachshund::slipper::Slipper => begin_slipper, step_slipper;
    Burrow: dachshund::burrow::Burrow => begin_burrow, step_burrow;
    UTurn: dachshund::u_turn::UTurn => begin_u_turn, step_u_turn;
    Yap: dachshund::yap::Yap => begin_yap, step_yap;
    Trumpet: elephant::trumpet::Trumpet => begin_trumpet, step_trumpet;
    Shower: elephant::shower::Shower => begin_shower, step_shower;
    MouseScare: elephant::mouse_scare::MouseScare => begin_mouse_scare, step_mouse_scare;
    TrunkNap: elephant::trunk_nap::TrunkNap => begin_trunk_nap, step_trunk_nap;
    Rinse: raccoon::rinse::Rinse => begin_rinse, step_rinse;
    Rummage: raccoon::rummage::Rummage => begin_rummage, step_rummage;
    Stash: raccoon::stash::Stash => begin_stash, step_stash;
    Lookout: raccoon::lookout::Lookout => begin_lookout, step_lookout;
    Toboggan: penguin::toboggan::Toboggan => begin_toboggan, step_toboggan;
    Wipeout: penguin::wipeout::Wipeout => begin_wipeout, step_wipeout;
    Applause: penguin::applause::Applause => begin_applause, step_applause;
    Strut: penguin::strut::Strut => begin_strut, step_strut;
}

/// Итог шага сценки.
enum Progress<T> {
    Going(T),
    /// Доиграна: приложению — `StuntFinished`.
    Finished,
    /// Сорвалась: молча.
    Dropped,
}

impl<T> Progress<T> {
    fn map(self, wrap: impl FnOnce(T) -> Act) -> Progress<Act> {
        match self {
            Self::Going(state) => Progress::Going(wrap(state)),
            Self::Finished => Progress::Finished,
            Self::Dropped => Progress::Dropped,
        }
    }
}

/// Разовые действия набора, из которых выбирается случайное.
pub(super) fn ambient(character: &Character) -> Vec<ActionId> {
    character
        .action_names()
        .filter(|name| !NOT_AMBIENT.contains(name))
        .filter_map(|name| character.action_id(name))
        .collect()
}

impl Sim {
    /// `false` — сейчас не до сценок (персонажа нет, его несут, сценка уже идёт),
    /// сценка чужого вида, озорная без разрешения или ей не хватает условий.
    pub(super) fn stage(&mut self, stunt: Stunt) -> bool {
        let foreign = stunt
            .cast()
            .is_some_and(|kind| Some(kind) != self.character.rig());
        let refused = foreign || (stunt.naughty() && !self.settings.naughty);
        if !self.alive() || self.dragging() || self.act.is_some() || refused {
            return false;
        }
        self.act = self.begin(stunt);
        if self.act.is_none() {
            // Начало могло что-то задать и отказаться
            self.wrap_stunt();
        }
        self.act.is_some()
    }

    /// Сценку обрывает то, что отнимает у персонажа управление: его взяли или убрали.
    pub(super) fn cancel_stunt(&mut self) {
        if matches!(self.act.take(), Some(Act::CursorHeist(_))) {
            self.grab.release();
        }
        self.wrap_stunt();
    }

    pub(super) fn advance_stunt(&mut self, dt: f32) {
        // Накладку позы шаг задаёт заново: сценка, потом состояния
        self.antic = Antic::default();
        self.reflex(dt);
        let Some(act) = self.act.take() else {
            self.idle_toward_stunt(dt);
            return;
        };
        if !self.alive() {
            self.act = Some(act);
            self.cancel_stunt();
            return;
        }
        let stunt = act.stunt();
        match self.step_act(act, dt) {
            Progress::Going(next) => self.act = Some(next),
            Progress::Finished => {
                self.wrap_stunt();
                push_event(&mut self.events, AgentEvent::StuntFinished(stunt));
            }
            Progress::Dropped => self.wrap_stunt(),
        }
    }

    /// Всё, что сценка держала на теле, гаснет вместе с ней.
    fn wrap_stunt(&mut self) {
        self.antic = Antic::default();
        self.drive = None;
        self.flock.clear();
    }

    /// Сам персонаж затевает сценку, только пока гуляет без дела.
    fn idle_toward_stunt(&mut self, dt: f32) {
        let [min, max] = self.stunt_gap;
        let free = matches!(self.brain.intent(), Intent::Wander | Intent::Rest);
        if !self.settings.stunts || max <= 0.0 || !free {
            return;
        }
        self.stunt_in -= dt;
        if self.stunt_in <= 0.0 {
            self.stunt_in = self.rng.range(min.max(0.0), max.max(min));
            self.stage_any();
        }
    }

    /// Случайная сценка своего вида или общая; не вышла одна — пробует следующую.
    fn stage_any(&mut self) -> bool {
        let roster: Vec<Stunt> = Stunt::roster(self.character.rig()).collect();
        let from = self.pick(roster.len());
        (0..roster.len()).any(|i| self.stage(roster[(from + i) % roster.len()]))
    }

    /// Случайное разовое действие набора или сценка.
    pub(super) fn surprise(&mut self) {
        if !self.alive() {
            return;
        }
        let roster: Vec<Stunt> = Stunt::roster(self.character.rig()).collect();
        let pick = self.pick(self.ambient.len() + roster.len());
        if let Some(stunt) = pick.checked_sub(self.ambient.len()).map(|i| roster[i])
            && self.stage(stunt)
        {
            return;
        }
        if let Some(action) = self.ambient.get(pick % self.ambient.len().max(1)).copied() {
            self.silent = None;
            self.brain.push(Directive::Perform(action));
        }
    }

    // --- общее для сценок ---

    /// Случайный индекс `0..len`; при пустом — 0.
    fn pick(&mut self, len: usize) -> usize {
        ((self.rng.unit() * len as f32) as usize).min(len.saturating_sub(1))
    }

    /// Место на том же мониторе в `distance` (логические px) от персонажа, не ближе `min`
    /// и не ближе `margin` к краю; `None`, если монитор тесен.
    fn spot_away(&mut self, distance: [f32; 2], min: f32, margin: f32) -> Option<Vec2> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin() + margin * self.ui;
        (0..SPOT_TRIES)
            .map(|_| {
                let direction = Vec2::from_angle(self.rng.range(0.0, TAU));
                let reach = self.rng.range(distance[0], distance[1]) * self.ui;
                area.clamp(self.body.pos + direction * reach, margin)
            })
            .find(|spot| spot.distance(self.body.pos) >= min * self.ui)
    }

    /// Остановиться там, где стоит: прошлый поход сценки больше не нужен.
    fn halt(&mut self) {
        self.brain.push(Directive::MoveTo(self.body.pos));
    }

    /// Действие — часть сценки, приложению о нём отдельно не сообщаем.
    fn perform_quietly(&mut self, action: ActionId) {
        self.silent = Some(action);
        self.brain.push(Directive::Perform(action));
    }

    /// Шутки записки — своего вида, у спрайтового набора — общие.
    fn notes(&self) -> &'static [&'static str] {
        match self.character.rig() {
            Some(RigKind::Goose) => goose::NOTES,
            Some(RigKind::Pig) => pig::NOTES,
            Some(RigKind::Dachshund) => dachshund::NOTES,
            Some(RigKind::Elephant) => elephant::NOTES,
            Some(RigKind::Raccoon) => raccoon::NOTES,
            Some(RigKind::Penguin) => penguin::NOTES,
            None => common::NOTES,
        }
    }
}
