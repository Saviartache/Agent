//! Предохранитель режима «персонаж носит системный курсор в клюве». Чистая логика:
//! курсор двигает платформенный слой, здесь — только решение, можно ли и куда.

use crate::config::CursorGrabConfig;
use crate::math::Vec2;

/// Дольше этого не держим при любых настройках, с.
const MAX_HOLD_SECS: f32 = 6.0;
/// Короче этого паузу не делаем, с.
const MIN_COOLDOWN_SECS: f32 = 10.0;
/// Курсор убежал от места, куда его поставил прошлый шаг, — пользователь тянет назад, px.
const PULLBACK_PX: f32 = 24.0;
/// Быстрее курсор не движется: поспевает за бегущим персонажем, но не прыгает, px/с.
const CARRY_SPEED: f32 = 900.0;

#[derive(Clone, Copy, Debug)]
struct Hold {
    left: f32,
    /// Куда прошлый шаг поставил курсор.
    placed: Vec2,
}

#[derive(Clone, Debug)]
pub struct CursorGrab {
    config: CursorGrabConfig,
    hold: Option<Hold>,
    cooldown_left: f32,
}

fn finite(v: Vec2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

impl CursorGrab {
    pub fn new(config: CursorGrabConfig) -> Self {
        Self {
            config,
            hold: None,
            cooldown_left: 0.0,
        }
    }

    /// Срок одного захвата, с: настройка, но не дольше предела.
    pub fn limit(&self) -> f32 {
        self.config.max_secs.clamp(0.0, MAX_HOLD_SECS)
    }

    /// Захват сейчас возможен: включён, не держим и пауза прошла.
    pub fn ready(&self) -> bool {
        self.config.enabled
            && self.hold.is_none()
            && self.cooldown_left <= 0.0
            && self.limit() > 0.0
    }

    pub fn try_begin(&mut self, cursor: Vec2) -> bool {
        if !self.ready() || !finite(cursor) {
            return false;
        }
        self.hold = Some(Hold {
            left: self.limit(),
            placed: cursor,
        });
        true
    }

    /// Пока держим — новая позиция курсора по пути к `carry_to` (клюв, рыло);
    /// `None` — не держим или только что отпустили.
    pub fn step(
        &mut self,
        dt: f32,
        cursor: Option<Vec2>,
        carry_to: Vec2,
        user_input: bool,
        permitted: bool,
    ) -> Option<Vec2> {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        let Some(hold) = self.hold.as_mut() else {
            self.cooldown_left = (self.cooldown_left - dt).max(0.0);
            return None;
        };
        let cursor = cursor.filter(|c| finite(*c) && finite(carry_to));
        let Some(cursor) = cursor.filter(|_| permitted && !user_input) else {
            self.release();
            return None;
        };
        hold.left -= dt;
        if cursor.distance(hold.placed) > PULLBACK_PX || hold.left <= 0.0 {
            self.release();
            return None;
        }
        let to = carry_to - cursor;
        let next = cursor + to.normalized() * (CARRY_SPEED * dt).min(to.length());
        hold.placed = next;
        Some(next)
    }

    pub fn holding(&self) -> bool {
        self.hold.is_some()
    }

    pub fn release(&mut self) {
        if self.hold.take().is_some() {
            self.cooldown_left = self.config.cooldown_secs.max(MIN_COOLDOWN_SECS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn grab(max_secs: f32, cooldown_secs: f32) -> CursorGrab {
        CursorGrab::new(CursorGrabConfig {
            enabled: true,
            max_secs,
            cooldown_secs,
        })
    }

    /// Держим без помех, возвращая секунды до отпускания.
    fn hold_duration(grab: &mut CursorGrab) -> f32 {
        let mut cursor = Vec2::ZERO;
        let mut secs = 0.0;
        while let Some(next) = grab.step(DT, Some(cursor), Vec2::new(1000.0, 0.0), false, true) {
            cursor = next;
            secs += DT;
            assert!(secs < 10.0);
        }
        secs
    }

    #[test]
    fn disabled_by_default_never_begins() {
        let mut grab = CursorGrab::new(CursorGrabConfig::default());
        assert!(!grab.try_begin(Vec2::ZERO));
        assert!(!grab.holding());
    }

    #[test]
    fn hold_is_capped_at_six_seconds() {
        let mut long = grab(60.0, 120.0);
        assert_eq!(long.limit(), 6.0);
        assert!(long.try_begin(Vec2::ZERO));
        assert!(hold_duration(&mut long) <= 6.0 + DT);
        let mut short = grab(0.5, 120.0);
        assert!(short.try_begin(Vec2::ZERO));
        assert!(hold_duration(&mut short) <= 0.5 + DT);
    }

    #[test]
    fn second_begin_while_holding_is_refused() {
        let mut grab = grab(1.5, 120.0);
        assert!(grab.try_begin(Vec2::ZERO));
        assert!(!grab.try_begin(Vec2::ZERO));
    }

    #[test]
    fn releases_on_user_input_unpermitted_or_missing_cursor() {
        let toward = Vec2::new(500.0, 0.0);
        for (cursor, input, permitted) in [
            (Some(Vec2::ZERO), true, true),
            (Some(Vec2::ZERO), false, false),
            (None, false, true),
        ] {
            let mut grab = grab(1.5, 120.0);
            assert!(grab.try_begin(Vec2::ZERO));
            assert_eq!(grab.step(DT, cursor, toward, input, permitted), None);
            assert!(!grab.holding());
        }
    }

    #[test]
    fn releases_when_user_pulls_back() {
        let mut grab = grab(1.5, 120.0);
        assert!(grab.try_begin(Vec2::ZERO));
        let toward = Vec2::new(500.0, 0.0);
        let placed = grab
            .step(DT, Some(Vec2::ZERO), toward, false, true)
            .unwrap();
        let yanked = Vec2::new(placed.x - 40.0, 0.0);
        assert_eq!(grab.step(DT, Some(yanked), toward, false, true), None);
        assert!(!grab.holding());
    }

    #[test]
    fn cooldown_is_at_least_ten_seconds_and_runs_on_dt_only() {
        let mut grab = grab(1.0, 0.0);
        assert!(grab.try_begin(Vec2::ZERO));
        hold_duration(&mut grab);
        assert!(!grab.try_begin(Vec2::ZERO));
        grab.step(9.0, None, Vec2::ZERO, false, true);
        assert!(!grab.try_begin(Vec2::ZERO));
        grab.step(1.5, None, Vec2::ZERO, false, true);
        assert!(grab.try_begin(Vec2::ZERO));
    }

    #[test]
    fn configured_cooldown_above_minimum_is_honored() {
        let mut grab = grab(1.0, 120.0);
        assert!(grab.try_begin(Vec2::ZERO));
        grab.release();
        grab.step(60.0, None, Vec2::ZERO, false, true);
        assert!(!grab.try_begin(Vec2::ZERO));
        grab.step(61.0, None, Vec2::ZERO, false, true);
        assert!(grab.try_begin(Vec2::ZERO));
    }

    #[test]
    fn carry_is_speed_capped_and_only_toward_the_goal() {
        let mut grab = grab(3.0, 120.0);
        let mut cursor = Vec2::new(100.0, 100.0);
        let goal = Vec2::new(300.0, 100.0);
        assert!(grab.try_begin(cursor));
        while let Some(next) = grab.step(DT, Some(cursor), goal, false, true) {
            assert!(cursor.distance(next) <= CARRY_SPEED * DT + 1e-3);
            assert!(next.distance(goal) <= cursor.distance(goal));
            assert_eq!(next.y, 100.0);
            cursor = next;
        }
        assert!(cursor.x > 100.0 && cursor.x <= goal.x);
    }

    #[test]
    fn huge_dt_never_extends_the_hold() {
        let mut grab = grab(1.5, 120.0);
        assert!(grab.try_begin(Vec2::ZERO));
        assert_eq!(
            grab.step(1e6, Some(Vec2::ZERO), Vec2::new(9.0, 0.0), false, true),
            None
        );
        assert!(!grab.holding());
    }
}
