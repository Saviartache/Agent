//! Гусята: три малыша идут за гусем след в след через экран и уходят за ним за край.
//! Следов не оставляют: это позы для окна эффектов, а не тело гуся.

use super::super::Progress;
use crate::behavior::Directive;
use crate::math::Vec2;
use crate::rig::{Antic, Creature, Gesture, RigInput, RigKind};
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const KIDS: usize = 3;
const SIZE: f32 = 0.45;
/// Гусёнок i идёт на столько позади по пути гуся, design px.
const GAP: f32 = 30.0;
const CROSS_DISTANCE: [f32; 2] = [450.0, 550.0];
const CROSS_MIN_DISTANCE: f32 = 300.0;
const EDGE_MARGIN: f32 = 40.0;
/// Путь гуся запоминается точками через столько px.
const PATH_STEP: f32 = 8.0;
/// Гусята доходят до точки за гусем у края и пропадают.
const BEYOND_EDGE: f32 = 45.0;
/// Скорость гусят после остановки гуся, design px/с.
const CATCH_UP_SPEED: f32 = 90.0;
/// Бегом считается скорость выше этой доли бега набора.
const RUN_SHARE: f32 = 0.6;
const TIMEOUT_SECS: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Cross(Vec2),
    Leave(Vec2),
    /// Гусь встал у края, гусята дошагивают до точки за ним.
    Exit(Vec2),
}

#[derive(Debug)]
struct Kid {
    body: Creature,
    /// Длина пути от гусёнка до конца пути, физические px.
    behind: f32,
}

#[derive(Debug)]
pub(in crate::sim) struct Goslings {
    kids: Vec<Kid>,
    /// Путь гуся: от старого к новому.
    path: Vec<Vec2>,
    phase: Phase,
    left: f32,
}

/// Точка пути в `dist` от его конца `tip` и направление хода там.
fn trail_at(path: &[Vec2], tip: Vec2, dist: f32) -> (Vec2, Vec2) {
    let mut from = tip;
    let mut left = dist;
    let mut heading = Vec2::new(1.0, 0.0);
    for &point in path.iter().rev() {
        let seg = from.distance(point);
        if seg > 0.0 {
            heading = (from - point).normalized();
            if seg >= left {
                return (from.lerp(point, left / seg), heading);
            }
        }
        left -= seg;
        from = point;
    }
    (from, heading)
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_goslings(&mut self) -> Option<Goslings> {
        let to = self.spot_away(CROSS_DISTANCE, CROSS_MIN_DISTANCE, EDGE_MARGIN)?;
        let (pos, heading) = (self.body.pos, self.body.heading);
        // Сначала путь — прямая позади гуся: малыши стоят строем
        let step = PATH_STEP * self.ui;
        let marks = (KIDS as f32 * GAP * self.sprite / step).ceil() as usize + 2;
        let path: Vec<Vec2> = (0..=marks)
            .rev()
            .map(|k| pos - heading * (k as f32 * step))
            .collect();
        let kids = (0..KIDS)
            .map(|i| {
                let behind = (i + 1) as f32 * GAP * self.sprite;
                let (at, _) = trail_at(&path, pos, behind);
                Kid {
                    body: Creature::new(RigKind::Goose, at, heading, self.sprite * SIZE),
                    behind,
                }
            })
            .collect();
        self.brain.push(Directive::MoveTo(to));
        Some(Goslings {
            kids,
            path,
            phase: Phase::Cross(to),
            left: TIMEOUT_SECS,
        })
    }

    pub(in crate::sim::stunt) fn step_goslings(
        &mut self,
        mut act: Goslings,
        dt: f32,
    ) -> Progress<Goslings> {
        act.left -= dt;
        if act.left <= 0.0 {
            self.halt();
            return Progress::Dropped;
        }
        let at = |sim: &Sim, to: Vec2| sim.body.pos.distance(to) <= 8.0 * sim.ui;
        let mut tip = self.body.pos;
        match act.phase {
            Phase::Cross(to) if at(self, to) => {
                let edge = self.nearest_edge();
                self.brain.push(Directive::MoveTo(edge));
                act.phase = Phase::Leave(edge);
            }
            Phase::Leave(to) if at(self, to) => {
                // Гусь встал: гусята идут к точке за ним
                self.halt();
                act.path.push(self.body.pos);
                let (_, heading) = trail_at(&act.path, self.body.pos, f32::MAX);
                let beyond = self.body.pos + heading * (BEYOND_EDGE * self.ui);
                for kid in &mut act.kids {
                    kid.behind += BEYOND_EDGE * self.ui;
                }
                act.phase = Phase::Exit(beyond);
            }
            _ => {}
        }
        if let Phase::Exit(beyond) = act.phase {
            tip = beyond;
            let step = CATCH_UP_SPEED * self.sprite * dt;
            act.kids.retain_mut(|kid| {
                kid.behind -= step;
                kid.behind > 0.0
            });
            if act.kids.is_empty() {
                return Progress::Finished;
            }
        } else if act
            .path
            .last()
            .is_none_or(|last| last.distance(self.body.pos) >= PATH_STEP * self.ui)
        {
            act.path.push(self.body.pos);
        }
        self.walk_flock(&mut act.kids, &act.path, tip, dt);
        Progress::Going(act)
    }

    /// Гусята идут по пути гуся, каждый на свой отрезок позади; позы — в стайку.
    fn walk_flock(&mut self, kids: &mut [Kid], path: &[Vec2], tip: Vec2, dt: f32) {
        let run = self.character.motion().run_speed * self.sprite * RUN_SHARE;
        let mut steps = Vec::new();
        self.flock.clear();
        for kid in kids {
            let (at, heading) = trail_at(path, tip, kid.behind);
            let vel = (at - kid.body.pose().pos) * (1.0 / dt.max(1e-3));
            let gesture = match vel.length() {
                v if v > run => Gesture::Run,
                v if v > 5.0 => Gesture::Walk,
                _ => Gesture::Idle,
            };
            let input = RigInput {
                pos: at,
                vel,
                heading,
                look_at: None,
                gesture,
                progress: 0.0,
                scale: self.sprite * SIZE,
                grounded: true,
                antic: Antic::default(),
            };
            kid.body.update(dt, &input, &mut self.rng, &mut steps);
            self.flock.push(kid.body.pose().clone());
        }
    }

    /// Ближайшая к гусю точка у края монитора.
    fn nearest_edge(&self) -> Vec2 {
        let area = self.bounds.nearest(self.body.pos);
        let p = self.body.pos;
        [
            Vec2::new(area.min.x, p.y),
            Vec2::new(area.max.x, p.y),
            Vec2::new(p.x, area.min.y),
            Vec2::new(p.x, area.max.y),
        ]
        .into_iter()
        .min_by(|a, b| p.distance(*a).total_cmp(&p.distance(*b)))
        .map_or(p, |edge| self.bounds.clamp(edge, self.edge_margin()))
    }
}
