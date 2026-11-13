//! Бульдозер: толкает рылом жёлудь через весь экран.

use std::f32::consts::TAU;

use super::super::Progress;
use super::snack::SNOUT_REACH;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::rig::GROUND_SQUASH;
use crate::sim::Sim;

/// Расстояния — логические px (design px — у тела), время — с.
const PUSH_DISTANCE: [f32; 2] = [400.0, 600.0];
const SPOT_TRIES: usize = 8;
/// Жёлудь у пятачка, design px.
const GRIP: f32 = 16.0;
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
        let margin = self.edge_margin() + SNOUT_REACH * self.sprite;
        // Сперва — куда уже смотрит
        let (dir, goal) = (0..SPOT_TRIES).find_map(|i| {
            let dir = if i == 0 {
                self.body.heading.normalized()
            } else {
                Vec2::from_angle(self.rng.range(0.0, TAU))
            };
            let at = self.snout(dir);
            let goal = at + dir * (self.rng.range(PUSH_DISTANCE[0], PUSH_DISTANCE[1]) * self.ui);
            (area.clamp(goal, margin) == goal).then_some((dir, goal))
        })?;
        // Жёлудь сразу у пятачка: свинка поворачивается и толкает
        self.body.heading = dir;
        let acorn = self
            .props
            .spawn(PropKind::Acorn, self.snout(dir), 0.0, self.ui, ACORN_TTL);
        self.brain
            .push(Directive::MoveTo(goal - (self.snout(dir) - self.body.pos)));
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
                self.snout_down(1.0);
                let snout = self.snout(act.dir);
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

    /// Где лежит жёлудь перед опущенным пятачком, когда свинка идёт по `dir`.
    fn snout(&self, dir: Vec2) -> Vec2 {
        let ahead = Vec2::new(dir.x, dir.y * GROUND_SQUASH);
        self.body.pos + ahead * (SNOUT_REACH * self.sprite + PropKind::Acorn.radius() * self.ui)
    }
}
