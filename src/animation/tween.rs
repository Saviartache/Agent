//! Программное движение: значение, идущее от `from` к `to` по кривой.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    InQuad,
    OutQuad,
    InOutCubic,
    /// Перелёт цели с возвратом.
    OutBack,
}

impl Easing {
    /// `t` вне `[0, 1]` прижимается к краям.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::InQuad => t * t,
            Self::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::InOutCubic if t < 0.5 => 4.0 * t * t * t,
            Self::InOutCubic => 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0,
            Self::OutBack => {
                const OVERSHOOT: f32 = 1.70158;
                let u = t - 1.0;
                1.0 + (OVERSHOOT + 1.0) * u * u * u + OVERSHOOT * u * u
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    duration: f32,
    elapsed: f32,
    easing: Easing,
}

impl Tween {
    pub fn new(from: f32, to: f32, duration: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: 0.0,
            easing,
        }
    }

    pub fn advance(&mut self, dt: f32) -> f32 {
        self.elapsed = (self.elapsed + dt.max(0.0)).min(self.duration.max(0.0));
        self.value()
    }

    pub fn value(&self) -> f32 {
        if self.finished() {
            return self.to;
        }
        let t = self.easing.apply(self.elapsed / self.duration);
        self.from + (self.to - self.from) * t
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_easing_starts_at_zero_and_ends_at_one() {
        for easing in [
            Easing::Linear,
            Easing::InQuad,
            Easing::OutQuad,
            Easing::InOutCubic,
            Easing::OutBack,
        ] {
            assert!(easing.apply(0.0).abs() < 1e-5, "{easing:?}");
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-5, "{easing:?}");
        }
    }

    #[test]
    fn huge_step_lands_exactly_on_target() {
        let mut tween = Tween::new(2.0, 10.0, 0.5, Easing::OutBack);
        assert_eq!(tween.advance(1e6), 10.0);
        assert!(tween.finished());
    }

    #[test]
    fn zero_duration_is_finished_at_once() {
        let tween = Tween::new(0.0, 1.0, 0.0, Easing::Linear);
        assert_eq!(tween.value(), 1.0);
    }
}
