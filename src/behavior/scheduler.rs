//! Планировщик намерений свободного режима: взвешенный выбор среди разрешённых,
//! возможных и не на паузе; время приходит аргументом.

use crate::config::BehaviorConfig;
use crate::rng::Rng;

/// Короче намерение не бывает, с: защита от нулей в манифесте.
const MIN_SECS: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntentKind {
    Wander,
    Rest,
    ObserveCursor,
    ApproachCursor,
    ChaseCursor,
    Nap,
}

const KINDS: [IntentKind; 6] = [
    IntentKind::Wander,
    IntentKind::Rest,
    IntentKind::ObserveCursor,
    IntentKind::ApproachCursor,
    IntentKind::ChaseCursor,
    IntentKind::Nap,
];

impl IntentKind {
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    weight: f32,
    secs: [f32; 2],
    cooldown: f32,
    enabled: bool,
    ready_at: f32,
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    slots: [Slot; 6],
    last: Option<IntentKind>,
}

impl Scheduler {
    pub fn new(config: &BehaviorConfig) -> Self {
        let slot = |weight, secs, cooldown| Slot {
            weight,
            secs,
            cooldown,
            enabled: true,
            ready_at: 0.0,
        };
        Self {
            slots: [
                slot(config.wander_weight, config.wander_secs, 0.0),
                slot(config.rest_weight, config.rest_secs, 0.0),
                slot(config.observe_cursor_weight, config.observe_secs, 0.0),
                slot(
                    config.approach_cursor_weight,
                    config.approach_secs,
                    config.approach_cooldown_secs,
                ),
                slot(
                    config.chase_cursor_weight,
                    config.chase_secs,
                    config.chase_cooldown_secs,
                ),
                slot(config.nap_weight, config.nap_secs, config.nap_cooldown_secs),
            ],
            last: None,
        }
    }

    pub fn set_enabled(&mut self, kind: IntentKind, enabled: bool) {
        self.slots[kind.index()].enabled = enabled;
    }

    /// Вид и его длительность, с. Без кандидатов — `Rest`.
    pub fn pick(
        &mut self,
        now: f32,
        possible: impl Fn(IntentKind) -> bool,
        rng: &mut Rng,
    ) -> (IntentKind, f32) {
        let weights = |skip_last: bool| {
            KINDS.map(|kind| {
                let slot = &self.slots[kind.index()];
                let repeat = skip_last && self.last == Some(kind) && kind != IntentKind::Wander;
                if slot.enabled && now >= slot.ready_at && !repeat && possible(kind) {
                    slot.weight
                } else {
                    0.0
                }
            })
        };
        let kind = rng
            .weighted(&weights(true))
            .or_else(|| rng.weighted(&weights(false)))
            .map_or(IntentKind::Rest, |index| KINDS[index]);
        self.last = Some(kind);
        let [min, max] = self.slots[kind.index()].secs;
        let min = min.max(MIN_SECS);
        (kind, rng.range(min, max.max(min)))
    }

    /// Намерение закончилось: вид уходит на паузу.
    pub fn finish(&mut self, kind: IntentKind, now: f32) {
        let slot = &mut self.slots[kind.index()];
        slot.ready_at = now + slot.cooldown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick_many(scheduler: &mut Scheduler, count: usize, possible: impl Fn(IntentKind) -> bool) {
        let mut rng = Rng::new(5);
        let mut now = 0.0;
        let mut previous = None;
        for _ in 0..count {
            let (kind, secs) = scheduler.pick(now, &possible, &mut rng);
            assert!(possible(kind) || kind == IntentKind::Rest);
            assert!(secs >= MIN_SECS);
            assert!(previous != Some(kind) || kind == IntentKind::Wander);
            previous = Some(kind);
            scheduler.finish(kind, now);
            now += secs;
        }
    }

    #[test]
    fn never_repeats_non_wander_kind() {
        let mut scheduler = Scheduler::new(&BehaviorConfig::default());
        pick_many(&mut scheduler, 500, |_| true);
    }

    #[test]
    fn respects_possibility() {
        let mut scheduler = Scheduler::new(&BehaviorConfig::default());
        let mut rng = Rng::new(2);
        for _ in 0..300 {
            let (kind, _) =
                scheduler.pick(1e6, |k| !matches!(k, IntentKind::ChaseCursor), &mut rng);
            assert_ne!(kind, IntentKind::ChaseCursor);
        }
    }

    #[test]
    fn cooldown_blocks_until_elapsed() {
        let config = BehaviorConfig {
            wander_weight: 0.0,
            rest_weight: 0.0,
            nap_weight: 1.0,
            nap_cooldown_secs: 50.0,
            ..BehaviorConfig::default()
        };
        let mut scheduler = Scheduler::new(&config);
        scheduler.set_enabled(IntentKind::ObserveCursor, false);
        scheduler.set_enabled(IntentKind::ApproachCursor, false);
        scheduler.set_enabled(IntentKind::ChaseCursor, false);
        let mut rng = Rng::new(1);
        assert_eq!(scheduler.pick(0.0, |_| true, &mut rng).0, IntentKind::Nap);
        scheduler.finish(IntentKind::Nap, 10.0);
        assert_eq!(scheduler.pick(59.0, |_| true, &mut rng).0, IntentKind::Rest);
        assert_eq!(scheduler.pick(60.0, |_| true, &mut rng).0, IntentKind::Nap);
    }

    #[test]
    fn disabled_kind_is_never_picked() {
        let mut scheduler = Scheduler::new(&BehaviorConfig::default());
        scheduler.set_enabled(IntentKind::Wander, false);
        let mut rng = Rng::new(9);
        assert!((0..300).all(|_| scheduler.pick(0.0, |_| true, &mut rng).0 != IntentKind::Wander));
        scheduler.set_enabled(IntentKind::Wander, true);
        assert!((0..300).any(|_| scheduler.pick(0.0, |_| true, &mut rng).0 == IntentKind::Wander));
    }

    #[test]
    fn falls_back_to_rest_when_nothing_is_left() {
        let mut scheduler = Scheduler::new(&BehaviorConfig::default());
        for kind in KINDS {
            scheduler.set_enabled(kind, false);
        }
        let mut rng = Rng::new(4);
        assert_eq!(scheduler.pick(0.0, |_| true, &mut rng).0, IntentKind::Rest);
    }

    #[test]
    fn same_seed_same_picks() {
        let run = |seed| {
            let mut scheduler = Scheduler::new(&BehaviorConfig::default());
            let mut rng = Rng::new(seed);
            (0..50)
                .map(|i| scheduler.pick(i as f32 * 30.0, |_| true, &mut rng))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(8), run(8));
        assert_ne!(run(8), run(9));
    }
}
