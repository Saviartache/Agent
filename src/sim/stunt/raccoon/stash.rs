//! Заначка: енот повернулся к зрителю с невинным видом, за спиной торчит записка.
//! Отдаёт её, только когда по нему щёлкнут, — записка ложится обычной карточкой;
//! не щёлкнули — уносит с собой. Только на пустой экран, как `Note`.

use std::f32::consts::TAU;
use std::time::Duration;

use super::super::Progress;
use crate::command::Notification;
use crate::effects::particles::Burst;
use crate::effects::props::{PropId, PropKind};
use crate::math::Vec2;
use crate::rig::GROUND_SQUASH;
use crate::sim::Sim;

/// Время — с, расстояния — design px от точки под телом.
const WAIT_SECS: f32 = 12.0;
/// Записка за спиной: назад, вбок и вверх.
const BEHIND: [f32; 3] = [7.0, 9.0, 9.0];
const PAPER_TTL: f32 = WAIT_SECS + 1.0;
const NOTE_TTL: Duration = Duration::from_secs(5);
const DELIVER_SECS: f32 = 1.0;
/// Невинно покачивает головой и косится на спрятанное.
const SWAY_HZ: f32 = 0.6;
const PEEK_EVERY: f32 = 3.0;
const PEEK_SECS: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Hide(f32),
    Give(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Stash {
    paper: PropId,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_stash(&mut self) -> Option<Stash> {
        if !self.items.is_empty() {
            return None;
        }
        let paper = self
            .props
            .spawn(PropKind::Paper, self.body.pos, 0.0, self.ui, PAPER_TTL);
        self.halt();
        self.clicked = false;
        self.hide_paper(paper);
        Some(Stash {
            paper,
            phase: Phase::Hide(0.0),
        })
    }

    pub(in crate::sim::stunt) fn step_stash(&mut self, act: Stash, dt: f32) -> Progress<Stash> {
        let phase = match act.phase {
            Phase::Hide(_) if self.clicked => {
                self.props.remove(act.paper);
                let notes = self.notes();
                let text = notes[self.pick(notes.len())];
                self.notify(Notification::info(text).ttl(NOTE_TTL));
                self.burst(Burst::Hearts, self.head());
                Phase::Give(DELIVER_SECS)
            }
            Phase::Hide(t) if t >= WAIT_SECS => {
                self.props.remove(act.paper);
                return Progress::Finished;
            }
            Phase::Hide(t) => {
                self.innocent(t);
                self.hide_paper(act.paper);
                Phase::Hide(t + dt)
            }
            Phase::Give(left) if left > dt => Phase::Give(left - dt),
            Phase::Give(_) => return Progress::Finished,
        };
        Progress::Going(Stash { phase, ..act })
    }

    /// Лицом к зрителю, голова покачивается, изредка косится назад.
    fn innocent(&mut self, t: f32) {
        self.body.heading = Vec2::new(0.0, 1.0);
        self.antic.head_yaw = 0.3 * (TAU * SWAY_HZ * t).sin();
        if t % PEEK_EVERY > PEEK_EVERY - PEEK_SECS {
            self.antic.head_yaw = 1.0;
        }
        self.antic.lids = 0.4;
    }

    /// Записка за спиной: уголок торчит из-за бока.
    fn hide_paper(&mut self, id: PropId) {
        let heading = self.body.heading.normalized();
        let flat = |v: Vec2| Vec2::new(v.x, v.y * GROUND_SQUASH) * self.sprite;
        let right = Vec2::new(-heading.y, heading.x);
        let ground = self.body.pos - flat(heading) * BEHIND[0] + flat(right) * BEHIND[1];
        let s = self.sprite;
        if let Some(paper) = self.props.get_mut(id) {
            paper.pos = ground;
            paper.z = BEHIND[2] * s;
            paper.vel = Vec2::ZERO;
            paper.vz = 0.0;
            paper.angle = 0.6;
        }
    }
}
