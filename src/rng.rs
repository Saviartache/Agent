//! Детерминированный генератор: одно зерно — одна и та же жизнь персонажа.

/// SplitMix64.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Равномерно в `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Равномерно в `[min, max)`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.unit()
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.unit() < probability
    }

    /// Индекс по весам; неположительные веса не выбираются.
    pub fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.range(0.0, total);
        let mut last = None;
        for (index, weight) in weights.iter().enumerate() {
            if *weight <= 0.0 {
                continue;
            }
            if roll < *weight {
                return Some(index);
            }
            roll -= weight;
            last = Some(index);
        }
        // Остаток округления
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        assert!((0..100).all(|_| a.next_u64() == b.next_u64()));
    }

    #[test]
    fn unit_stays_in_half_open_range() {
        let mut rng = Rng::new(1);
        assert!((0..10_000).all(|_| (0.0..1.0).contains(&rng.unit())));
    }

    #[test]
    fn weighted_skips_non_positive() {
        let mut rng = Rng::new(3);
        assert!((0..1000).all(|_| {
            rng.weighted(&[0.0, 2.0, -1.0, 1.0])
                .is_some_and(|i| i == 1 || i == 3)
        }));
        assert_eq!(rng.weighted(&[0.0, -1.0]), None);
        assert_eq!(rng.weighted(&[]), None);
    }
}
