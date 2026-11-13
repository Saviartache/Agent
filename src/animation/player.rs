//! Проигрыватель клипа. Время приходит от вызывающего, поэтому скорость
//! не зависит от частоты вызовов; кадры не смешиваются.

use crate::{action::ClipId, animation::clip::Clip, assets::Character};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimEvent {
    /// `marker` — индекс в `clip.markers`.
    Marker {
        clip: ClipId,
        marker: u16,
    },
    Finished {
        clip: ClipId,
    },
}

#[derive(Clone, Debug)]
pub struct Player {
    clip: ClipId,
    priority: u8,
    speed: f32,
    /// Секунды от начала клипа.
    time: f64,
    frame: usize,
    /// Маркеры нулевого кадра ещё не выданы.
    fresh: bool,
    finished: bool,
    // Известно после первого `advance`; до него клип считается зацикленным.
    looping: bool,
}

impl Player {
    pub fn new(clip: ClipId) -> Self {
        Self {
            clip,
            priority: 0,
            speed: 1.0,
            time: 0.0,
            frame: 0,
            fresh: true,
            finished: false,
            looping: true,
        }
    }

    /// Тот же клип ещё идёт — продолжает (`true`). Незавершённый одиночный клип
    /// с большим приоритетом не прерывается (`false`). Иначе — переключает и начинает заново.
    pub fn play(&mut self, clip: ClipId, priority: u8) -> bool {
        if self.finished {
            self.force(clip, priority);
            return true;
        }
        if clip == self.clip {
            return true;
        }
        if !self.looping && priority < self.priority {
            return false;
        }
        self.force(clip, priority);
        true
    }

    pub fn force(&mut self, clip: ClipId, priority: u8) {
        self.clip = clip;
        self.priority = priority;
        self.time = 0.0;
        self.frame = 0;
        self.fresh = true;
        self.finished = false;
        self.looping = true;
    }

    /// Множитель времени в `[0, 8]`; NaN — остановка.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = if speed.is_nan() {
            0.0
        } else {
            speed.clamp(0.0, 8.0)
        };
    }

    pub fn advance(&mut self, dt: f32, character: &Character, events: &mut Vec<AnimEvent>) {
        let clip = character.clip(self.clip);
        self.looping = clip.looping;
        let total = total_secs(clip);
        if self.finished || total <= 0.0 {
            return;
        }
        let step = if dt.is_finite() && dt > 0.0 {
            f64::from(dt) * f64::from(self.speed)
        } else {
            0.0
        };
        let from = self.time;
        let to = from + step;
        // Нижняя граница исключена: -1 включает маркеры нулевого кадра.
        let lo = if std::mem::take(&mut self.fresh) {
            -1.0
        } else {
            from
        };
        let mut emit = |lo: f64, hi: f64| emit_markers(clip, self.clip, lo, hi, events);

        if clip.looping {
            if step >= total {
                emit(lo, total);
                if lo >= 0.0 {
                    emit(-1.0, from);
                }
                self.time = to % total;
            } else if to >= total {
                emit(lo, total);
                self.time = to - total;
                emit(-1.0, self.time);
            } else {
                emit(lo, to);
                self.time = to;
            }
        } else if to >= total {
            emit(lo, total);
            self.time = total;
            self.finished = true;
            events.push(AnimEvent::Finished { clip: self.clip });
        } else {
            emit(lo, to);
            self.time = to;
        }
        self.frame = frame_at(clip, self.time);
    }

    pub fn clip(&self) -> ClipId {
        self.clip
    }

    /// Доля клипа, 0..=1; доигранный разовый клип — 1.
    pub fn progress(&self, character: &Character) -> f32 {
        let total = total_secs(character.clip(self.clip));
        if total > 0.0 {
            (self.time / total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }

    pub fn frame_index(&self) -> usize {
        self.frame
    }

    pub fn finished(&self) -> bool {
        self.finished
    }
}

fn total_secs(clip: &Clip) -> f64 {
    clip.frames.iter().map(|f| f64::from(f.duration)).sum()
}

fn frame_at(clip: &Clip, time: f64) -> usize {
    let mut end = 0.0;
    for (index, frame) in clip.frames.iter().enumerate() {
        end += f64::from(frame.duration);
        if time < end {
            return index;
        }
    }
    clip.frames.len().saturating_sub(1)
}

fn start_of(clip: &Clip, frame: u16) -> f64 {
    clip.frames[..usize::from(frame).min(clip.frames.len())]
        .iter()
        .map(|f| f64::from(f.duration))
        .sum()
}

/// Маркеры, чей момент лежит в `(lo, hi]`.
fn emit_markers(clip: &Clip, id: ClipId, lo: f64, hi: f64, events: &mut Vec<AnimEvent>) {
    for (index, marker) in clip.markers.iter().enumerate() {
        let at = start_of(clip, marker.frame);
        if at > lo && at <= hi {
            events.push(AnimEvent::Marker {
                clip: id,
                marker: index as u16,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::test_support;

    // loop: 0.25 + 0.25 + 0.5 с, маркеры a/b/c на кадрах 0/1/2; once: 0.5 + 0.5 с.
    const PACK: &str = r#"{
        "id": "t", "frame_size": [1, 1], "anchor": [0, 1],
        "animations": {
            "loop": { "file": "s.png", "loop": true, "frames": [
                { "x": 0, "y": 0, "w": 1, "h": 1, "duration_ms": 250 },
                { "x": 0, "y": 0, "w": 1, "h": 1, "duration_ms": 250 },
                { "x": 0, "y": 0, "w": 1, "h": 1, "duration_ms": 500 } ],
              "events": [ { "frame": 0, "name": "a" }, { "frame": 1, "name": "b" }, { "frame": 2, "name": "c" } ] },
            "once": { "file": "s.png", "frames": [
                { "x": 0, "y": 0, "w": 1, "h": 1, "duration_ms": 500 },
                { "x": 0, "y": 0, "w": 1, "h": 1, "duration_ms": 500 } ],
              "events": [ { "frame": 1, "name": "m" } ] }
        },
        "actions": { "idle": { "animation": "loop" }, "walk": { "animation": "once" } }
    }"#;

    struct Rig {
        character: Character,
        looped: ClipId,
        once: ClipId,
    }

    impl Rig {
        fn new() -> Self {
            let character = test_support::character(
                PACK,
                &[("s.png", test_support::png(1, 1, &[9, 9, 9, 255]))],
            )
            .unwrap();
            let clip = |name| character.action(character.action_id(name).unwrap()).clip;
            let (looped, once) = (clip("idle"), clip("walk"));
            Self {
                character,
                looped,
                once,
            }
        }

        fn run(&self, player: &mut Player, dt: f32) -> Vec<AnimEvent> {
            let mut events = Vec::new();
            player.advance(dt, &self.character, &mut events);
            events
        }
    }

    fn marker(clip: ClipId, marker: u16) -> AnimEvent {
        AnimEvent::Marker { clip, marker }
    }

    #[test]
    fn loop_wraps() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        rig.run(&mut player, 0.3);
        assert_eq!(player.frame_index(), 1);
        rig.run(&mut player, 0.75);
        assert_eq!(player.frame_index(), 0);
        assert!(!player.finished());
    }

    #[test]
    fn frames_follow_their_own_durations() {
        let rig = Rig::new();
        for (secs, frame) in [(0.24, 0), (0.25, 1), (0.49, 1), (0.5, 2), (0.99, 2)] {
            let mut player = Player::new(rig.looped);
            rig.run(&mut player, secs);
            assert_eq!(player.frame_index(), frame, "{secs}");
        }
    }

    #[test]
    fn progress_runs_through_clip_and_wraps_loops() {
        let rig = Rig::new();
        let mut player = Player::new(rig.once);
        rig.run(&mut player, 0.25);
        assert!((player.progress(&rig.character) - 0.25).abs() < 1e-5);
        rig.run(&mut player, 5.0);
        assert_eq!(player.progress(&rig.character), 1.0);
        let mut player = Player::new(rig.looped);
        rig.run(&mut player, 1.25);
        assert!((player.progress(&rig.character) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn one_shot_finishes_once() {
        let rig = Rig::new();
        let mut player = Player::new(rig.once);
        assert!(rig.run(&mut player, 0.6).contains(&marker(rig.once, 0)));
        assert!(!player.finished());
        let events = rig.run(&mut player, 0.5);
        assert_eq!(events, [AnimEvent::Finished { clip: rig.once }]);
        assert!(player.finished());
        assert_eq!(player.frame_index(), 1);
        assert!(rig.run(&mut player, 5.0).is_empty());
    }

    #[test]
    fn play_respects_priority_and_same_clip() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        rig.run(&mut player, 0.3);
        assert!(player.play(rig.looped, 0));
        assert_eq!(player.frame_index(), 1);

        player.force(rig.once, 3);
        rig.run(&mut player, 0.1);
        assert!(!player.play(rig.looped, 1));
        assert_eq!(player.clip(), rig.once);
        assert!(player.play(rig.looped, 3));
        assert_eq!(player.clip(), rig.looped);

        player.force(rig.once, 3);
        rig.run(&mut player, 2.0);
        assert!(player.finished());
        assert!(player.play(rig.once, 3));
        assert!(!player.finished());
        assert_eq!(player.frame_index(), 0);
    }

    #[test]
    fn loop_never_blocks_switching() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        player.force(rig.looped, 9);
        rig.run(&mut player, 0.1);
        assert!(player.play(rig.once, 0));
    }

    #[test]
    fn force_restarts_unconditionally() {
        let rig = Rig::new();
        let mut player = Player::new(rig.once);
        player.force(rig.once, 5);
        rig.run(&mut player, 0.7);
        player.force(rig.looped, 0);
        assert_eq!((player.clip(), player.frame_index()), (rig.looped, 0));
        assert!(!player.finished());
    }

    #[test]
    fn markers_come_in_order_and_start_marker_fires() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        let a = rig.run(&mut player, 0.9);
        assert_eq!(
            a,
            [
                marker(rig.looped, 0),
                marker(rig.looped, 1),
                marker(rig.looped, 2)
            ]
        );
        let b = rig.run(&mut player, 0.2);
        assert_eq!(b, [marker(rig.looped, 0)]);
    }

    #[test]
    fn huge_step_on_loop_wraps_without_looping() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        let events = rig.run(&mut player, 1_000_000.25);
        assert_eq!(events.len(), 3);
        assert_eq!(player.frame_index(), 1);
        assert!(!player.finished());
    }

    #[test]
    fn huge_step_on_one_shot_clamps() {
        let rig = Rig::new();
        let mut player = Player::new(rig.once);
        let events = rig.run(&mut player, 1e6);
        let finished = events
            .iter()
            .filter(|e| matches!(e, AnimEvent::Finished { .. }));
        assert_eq!(finished.count(), 1);
        assert_eq!(player.frame_index(), 1);
    }

    #[test]
    fn bad_steps_do_not_move_time() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            rig.run(&mut player, dt);
        }
        assert_eq!(player.frame_index(), 0);
        rig.run(&mut player, 0.3);
        assert_eq!(player.frame_index(), 1);
    }

    #[test]
    fn many_small_steps_equal_one_big_step() {
        let rig = Rig::new();
        let (mut small, mut big) = (Player::new(rig.looped), Player::new(rig.looped));
        let mut small_events = Vec::new();
        for _ in 0..48 {
            small.advance(1.0 / 64.0, &rig.character, &mut small_events);
        }
        let big_events = rig.run(&mut big, 0.75);
        assert_eq!(small.frame_index(), big.frame_index());
        assert_eq!(small_events, big_events);
    }

    #[test]
    fn speed_scales_time() {
        let rig = Rig::new();
        let mut player = Player::new(rig.looped);
        player.set_speed(2.0);
        rig.run(&mut player, 0.125);
        assert_eq!(player.frame_index(), 1);
        player.set_speed(0.0);
        rig.run(&mut player, 10.0);
        assert_eq!(player.frame_index(), 1);
        player.set_speed(f32::NAN);
        rig.run(&mut player, 10.0);
        assert_eq!(player.frame_index(), 1);
    }
}
