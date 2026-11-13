//! Мусорка: поодаль стоит ведро, енот подкрадывается, суёт в него морду и роется,
//! бумажки летят во все стороны и плавно опускаются; ведро и бумажки тают.

use std::f32::consts::TAU;

use super::super::Progress;
use super::PAWS_REACH;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, скорости — px/с, время — с.
const BIN_DISTANCE: [f32; 2] = [120.0, 240.0];
const BIN_MIN: f32 = 90.0;
const BIN_TTL: f32 = 30.0;
/// Ведро после сценки тает за столько.
const BIN_FADE: f32 = 2.0;
const REACH_SLACK: f32 = 9.0;
const WALK_SECS: f32 = 10.0;
const DIG_HZ: f32 = 3.0;
const PAPERS: u8 = 6;
const TOSS_GAP: f32 = 0.4;
const TOSS_SPEED: [f32; 2] = [50.0, 120.0];
const TOSS_UP: [f32; 2] = [120.0, 200.0];
const TOSS_SPIN: f32 = 6.0;
const PAPER_TTL: f32 = 8.0;
const PROUD_SECS: f32 = 1.2;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk(f32),
    Dig { t: f32, toss: f32, tossed: u8 },
    Proud(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Rummage {
    bin: PropId,
    face: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_rummage(&mut self) -> Option<Rummage> {
        let at = self.spot_away(BIN_DISTANCE, BIN_MIN, PropKind::Bucket.radius())?;
        let bin = self
            .props
            .spawn(PropKind::Bucket, at, 0.0, self.ui, BIN_TTL);
        let (face, spot) = self.reach_for(at, PAWS_REACH);
        self.brain.push(Directive::MoveTo(spot));
        Some(Rummage {
            bin,
            face,
            phase: Phase::Walk(WALK_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_rummage(
        &mut self,
        mut act: Rummage,
        dt: f32,
    ) -> Progress<Rummage> {
        let Some(bin) = self.props.get(act.bin).map(|bin| bin.pos) else {
            return Progress::Dropped;
        };
        act.phase = match act.phase {
            Phase::Walk(left) => {
                let (face, spot) = self.reach_for(bin, PAWS_REACH);
                if self.body.pos.distance(spot) <= REACH_SLACK * self.ui {
                    self.halt();
                    act.face = face;
                    Phase::Dig {
                        t: 0.0,
                        toss: 0.0,
                        tossed: 0,
                    }
                } else if left > dt {
                    Phase::Walk(left - dt)
                } else {
                    self.props.remove(act.bin);
                    return Progress::Dropped;
                }
            }
            Phase::Dig { t, toss, tossed } => {
                self.body.heading = act.face;
                let t = t + dt;
                self.paws_down(0.8 + 0.2 * (TAU * DIG_HZ * t).sin());
                self.antic.wobble = 0.7 * (TAU * DIG_HZ * 0.5 * t).sin();
                if toss > dt {
                    Phase::Dig {
                        t,
                        toss: toss - dt,
                        tossed,
                    }
                } else if tossed < PAPERS {
                    self.toss_paper(bin);
                    Phase::Dig {
                        t,
                        toss: TOSS_GAP,
                        tossed: tossed + 1,
                    }
                } else {
                    self.burst(Burst::Sparks, self.head());
                    Phase::Proud(PROUD_SECS)
                }
            }
            Phase::Proud(left) if left > dt => Phase::Proud(left - dt),
            Phase::Proud(_) => {
                if let Some(bin) = self.props.get_mut(act.bin) {
                    bin.expire_in(BIN_FADE);
                }
                return Progress::Finished;
            }
        };
        Progress::Going(act)
    }

    /// Бумажка из ведра: вверх и вбок, вертясь.
    fn toss_paper(&mut self, bin: Vec2) {
        let lip = PropKind::Bucket.radius() * 1.2 * self.ui;
        let way = Vec2::from_angle(self.rng.range(0.0, TAU));
        let speed = self.rng.range(TOSS_SPEED[0], TOSS_SPEED[1]) * self.ui;
        let up = self.rng.range(TOSS_UP[0], TOSS_UP[1]) * self.ui;
        let spin = self.rng.range(-TOSS_SPIN, TOSS_SPIN);
        let id = self
            .props
            .spawn(PropKind::Paper, bin, lip, self.ui, PAPER_TTL);
        if let Some(paper) = self.props.get_mut(id) {
            paper.vel = way * speed;
            paper.vz = up;
            paper.spin = spin;
        }
    }
}
