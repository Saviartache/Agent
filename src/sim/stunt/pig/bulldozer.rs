//! Бульдозер: толкает рылом жёлудь через весь экран.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px (design px — у тела), время — с.
const ACORN_DISTANCE: f32 = 80.0;
const PUSH_DISTANCE: [f32; 2] = [400.0, 600.0];
const SPOT_TRIES: usize = 8;
/// Жёлудь держится перед носом на таком расстоянии от тела, design px.
const SNOUT: f32 = 34.0;
/// Жёлудь у пятачка, design px.
const GRIP: f32 = 22.0;
/// Жёлудь едет чуть быстрее свинки, но пружина держит его у носа.
const PUSH_SPEED: f32 = 1.1;
const PUSH_SPRING: f32 = 4.0;
const ARRIVED: f32 = 20.0;
const DUST_GAP: f32 = 0.7;
const TIMEOUT: f32 = 20.0;
const CHEER_SECS: f32 = 1.5;
const ACORN_TTL: f32 = 60.0;
const ACORN_FADE: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Push { left: f32, dust: f32 },
    Cheer(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Bulldozer {
    acorn: PropId,
    /// Куда катить жёлудь.
    goal: Vec2,
    dir: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_bulldozer(&mut self) -> Option<Bulldozer> {
        let area = self.bounds.nearest(self.body.pos);
        let margin = self.edge_margin() + SNOUT * self.sprite;
        let (dir, acorn_at, goal) = (0..SPOT_TRIES).find_map(|_| {
            let dir = Vec2::from_angle(self.rng.range(0.0, TAU));
            let at = self.body.pos + dir * (ACORN_DISTANCE * self.ui);
            let goal = at + dir * (self.rng.range(PUSH_DISTANCE[0], PUSH_DISTANCE[1]) * self.ui);
            (area.clamp(goal, margin) == goal && area.clamp(at, margin) == at)
                .then_some((dir, at, goal))
        })?;
        let acorn = self
            .props
            .spawn(PropKind::Acorn, acorn_at, 0.0, self.ui, ACORN_TTL);
        self.brain
            .push(Directive::MoveTo(goal - dir * (SNOUT * self.sprite)));
        Some(Bulldozer {
            acorn,
            goal,
            dir,
            phase: Phase::Push {
                left: TIMEOUT,
                dust: DUST_GAP,
            },
        })
    }

    pub(in crate::sim::stunt) fn step_bulldozer(
        &mut self,
        mut act: Bulldozer,
        dt: f32,
    ) -> Progress<Bulldozer> {
        let Some(acorn) = self.props.get(act.acorn).copied() else {
            return Progress::Dropped;
        };
        act.phase = match act.phase {
            Phase::Push { left, dust } => {
                if acorn.pos.distance(act.goal) <= ARRIVED * self.ui {
                    self.halt();
                    self.perform_quietly(self.character.actions().celebrate);
                    if let Some(acorn) = self.props.get_mut(act.acorn) {
                        acorn.vel = Vec2::ZERO;
                        acorn.expire_in(ACORN_FADE);
                    }
                    return Progress::Going(Bulldozer {
                        phase: Phase::Cheer(CHEER_SECS),
                        ..act
                    });
                }
                if left <= dt {
                    self.props.remove(act.acorn);
                    return Progress::Dropped;
                }
                self.antic.neck_forward = 5.0;
                self.antic.neck_up = -5.0;
                self.antic.head_pitch = -0.6;
                let snout = self.body.pos + act.dir * (SNOUT * self.sprite);
                if acorn.pos.distance(snout) <= GRIP * self.sprite
                    && let Some(acorn) = self.props.get_mut(act.acorn)
                {
                    let speed = self.body.vel.length() * PUSH_SPEED;
                    acorn.vel = act.dir * speed + (snout - acorn.pos) * PUSH_SPRING;
                }
                let dust = if dust <= dt {
                    self.burst(Burst::Dust, snout);
                    DUST_GAP
                } else {
                    dust - dt
                };
                Phase::Push {
                    left: left - dt,
                    dust,
                }
            }
            Phase::Cheer(left) if left > dt => Phase::Cheer(left - dt),
            Phase::Cheer(_) => return Progress::Finished,
        };
        Progress::Going(act)
    }
}
