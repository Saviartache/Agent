//! Где курсор был недавно: по этому пути свинка идёт по запаху.

use std::collections::VecDeque;

use crate::math::Vec2;

const SAMPLE_SECS: f32 = 0.1;
/// Помнит столько последних секунд.
const KEEP_SECS: f32 = 6.0;

#[derive(Clone, Debug, Default)]
pub(super) struct Trail {
    /// От свежей к старой, через `SAMPLE_SECS`.
    points: VecDeque<Vec2>,
    since: f32,
}

impl Trail {
    /// Курсора нет — путь обрывается.
    pub fn record(&mut self, cursor: Option<Vec2>, dt: f32) {
        let Some(cursor) = cursor else {
            self.points.clear();
            return;
        };
        self.since += dt;
        if !self.points.is_empty() && self.since < SAMPLE_SECS {
            return;
        }
        self.since = 0.0;
        if self.points.len() == (KEEP_SECS / SAMPLE_SECS) as usize {
            self.points.pop_back();
        }
        self.points.push_front(cursor);
    }

    /// Где курсор был `secs` назад; путь короче — самое старое место; `None` — курсора нет.
    pub fn ago(&self, secs: f32) -> Option<Vec2> {
        let index = (secs.max(0.0) / SAMPLE_SECS).round() as usize;
        self.points
            .get(index.min(self.points.len().saturating_sub(1)))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_recent_path_and_forgets_the_old() {
        let mut trail = Trail::default();
        for i in 0..120 {
            trail.record(Some(Vec2::new(i as f32, 0.0)), SAMPLE_SECS);
        }
        assert_eq!(trail.ago(0.0), Some(Vec2::new(119.0, 0.0)));
        assert_eq!(trail.ago(1.0), Some(Vec2::new(109.0, 0.0)));
        let oldest = 120.0 - KEEP_SECS / SAMPLE_SECS;
        assert_eq!(trail.ago(KEEP_SECS + 1.0), Some(Vec2::new(oldest, 0.0)));
        trail.record(None, SAMPLE_SECS);
        assert_eq!(trail.ago(0.0), None);
    }
}
