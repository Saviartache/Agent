//! Каракули: гусь медленно обводит клювом узор рядом — сердечко, волну или спираль;
//! линия остаётся на земле под клювом и гаснет сама.

use std::f32::consts::TAU;

use super::super::Progress;
use super::crumbs::PECK_REACH;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const SIZE: f32 = 100.0;
/// Глубина узора на земле сжата, как у всего, что лежит.
const GROUND_SQUASH: f32 = 0.55;
const SPOT_DISTANCE: [f32; 2] = [110.0, 200.0];
const SPOT_MIN_DISTANCE: f32 = 80.0;
/// Точек узора на фигуру; гусь идёт по ним по одной.
const POINTS: usize = 40;
/// Следующая точка выдаётся, когда до этой осталось меньше.
const LEAD: f32 = 10.0;
/// Метки на земле ложатся не чаще этого шага.
const STAMP_GAP: f32 = 2.5;
const TIMEOUT_SECS: f32 = 20.0;
/// Клюв касается земли, когда ниже этого, design px.
const TOUCH: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Heart,
    Wave,
    Spiral,
}

#[derive(Debug)]
pub(in crate::sim) struct Scribble {
    path: Vec<Vec2>,
    /// Гусь ещё идёт к началу узора и не рисует.
    approach: bool,
    next: usize,
    last: Option<Vec2>,
    left: f32,
}

/// Точка узора `0..=1` в долях `SIZE`, середина узора в нуле.
fn pattern(shape: Shape, u: f32) -> Vec2 {
    match shape {
        Shape::Heart => {
            let t = std::f32::consts::PI + u * TAU;
            let x = 16.0 * t.sin().powi(3);
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            // Сердечко: остриё книзу, оно же начало
            Vec2::new(x / 34.0, -y / 34.0 + 0.1)
        }
        Shape::Wave => Vec2::new(u - 0.5, 0.18 * (u * TAU * 2.5).sin()),
        Shape::Spiral => {
            let angle = u * TAU * 2.2;
            Vec2::from_angle(angle) * (0.06 + 0.44 * u)
        }
    }
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_scribble(&mut self) -> Option<Scribble> {
        let center = self.spot_away(SPOT_DISTANCE, SPOT_MIN_DISTANCE, SIZE * 0.6)?;
        let shape = [Shape::Heart, Shape::Wave, Shape::Spiral][self.pick(3)];
        let path: Vec<Vec2> = (0..=POINTS)
            .map(|i| {
                let p = pattern(shape, i as f32 / POINTS as f32) * (SIZE * self.ui);
                center + Vec2::new(p.x, p.y * GROUND_SQUASH)
            })
            .collect();
        Some(Scribble {
            path,
            approach: true,
            next: 0,
            last: None,
            left: TIMEOUT_SECS,
        })
    }

    pub(in crate::sim::stunt) fn step_scribble(
        &mut self,
        mut act: Scribble,
        dt: f32,
    ) -> Progress<Scribble> {
        act.left -= dt;
        if act.left <= 0.0 {
            self.halt();
            return Progress::Dropped;
        }
        // Тело идёт так, чтобы клюв приходился на точку узора
        let (face, spot) = self.reach_for(act.path[act.next], PECK_REACH);
        self.brain.push(Directive::MoveTo(spot));
        let there = self.body.pos.distance(spot) <= LEAD * self.ui;
        if act.approach {
            act.approach = !there;
            return Progress::Going(act);
        }
        self.body.heading = face;
        self.peck_pose(1.0);
        let beak = self.muzzle_ground();
        // Чертит только клюв, коснувшийся земли
        if self.muzzle_height() > TOUCH * self.sprite {
            return Progress::Going(act);
        }
        act.last = Some(match act.last {
            Some(from) => self.stamp_between(from, beak),
            None => beak,
        });
        if there || beak.distance(act.path[act.next]) <= LEAD * self.ui {
            act.next += 1;
            if act.next >= act.path.len() {
                self.halt();
                return Progress::Finished;
            }
        }
        Progress::Going(act)
    }

    /// Метки по отрезку клюва: линия сплошная, даже когда клюв идёт быстро.
    /// Возвращает, докуда дотянулась линия.
    fn stamp_between(&mut self, from: Vec2, to: Vec2) -> Vec2 {
        let gap = STAMP_GAP * self.ui;
        let span = to - from;
        let steps = (span.length() / gap).floor() as usize;
        for i in 1..=steps {
            let at = from + span.normalized() * (gap * i as f32);
            self.scribble.stamp(at, span.normalized(), true, 0.0, 1.0);
        }
        from + span.normalized() * (gap * steps as f32)
    }
}
