//! Погоня: бежит за курсором, не хватая; догнал — тормозит юзом, задирает голову,
//! курсор убежал — снова бежит. Через несколько секунд сдаётся.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const SECS: f32 = 8.0;
/// Курсор ушёл от прошлой цели дальше этого — бежим за ним заново.
const RETARGET: f32 = 12.0;
/// Голова у курсора — догнал.
const CAUGHT: f32 = 30.0;
/// Курсор убежал от остановившегося — снова в погоню.
const ESCAPED: f32 = 80.0;
const BRAKE_FRICTION: f32 = 6.0;
const BRAKE_PITCH: f32 = 0.25;
const BRAKE_SECS: f32 = 0.5;
const DUST_GAP: f32 = 0.15;
const RAISE_SECS: f32 = 0.8;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Run { aim: Vec2 },
    Stop { t: f32, dust: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct Chase {
    left: f32,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_chase(&mut self) -> Option<Chase> {
        let cursor = self.cursor?;
        self.chase_toward(cursor);
        Some(Chase {
            left: SECS,
            phase: Phase::Run { aim: cursor },
        })
    }

    pub(in crate::sim::stunt) fn step_chase(&mut self, mut act: Chase, dt: f32) -> Progress<Chase> {
        let Some(cursor) = self.cursor else {
            self.halt();
            return Progress::Dropped;
        };
        act.left -= dt;
        if act.left <= 0.0 {
            self.halt();
            self.perform_quietly(self.character.actions().react);
            return Progress::Finished;
        }
        act.phase = match act.phase {
            Phase::Run { aim } => self.chase_run(cursor, aim),
            Phase::Stop { t, dust } => self.chase_stop(cursor, t + dt, dust - dt),
        };
        Progress::Going(act)
    }

    fn chase_run(&mut self, cursor: Vec2, aim: Vec2) -> Phase {
        let near = self
            .head()
            .distance(cursor)
            .min(self.body.pos.distance(cursor));
        if near <= CAUGHT * self.ui {
            self.drive = Some(Drive::new(self.body.vel).friction(BRAKE_FRICTION));
            self.burst(Burst::Dust, self.body.pos);
            return Phase::Stop {
                t: 0.0,
                dust: DUST_GAP,
            };
        }
        if aim.distance(cursor) <= RETARGET * self.ui {
            return Phase::Run { aim };
        }
        self.chase_toward(cursor);
        Phase::Run { aim: cursor }
    }

    fn chase_stop(&mut self, cursor: Vec2, t: f32, mut dust: f32) -> Phase {
        if self.body.pos.distance(cursor) > ESCAPED * self.ui {
            self.drive = None;
            self.chase_toward(cursor);
            return Phase::Run { aim: cursor };
        }
        if t < BRAKE_SECS && dust <= 0.0 {
            dust = DUST_GAP;
            self.burst(Burst::Dust, self.body.pos);
        }
        let brake = 1.0 - ((t - BRAKE_SECS * 0.5) / (BRAKE_SECS * 0.5)).clamp(0.0, 1.0);
        let raise = (t / RAISE_SECS).min(1.0);
        self.antic.pitch = BRAKE_PITCH * brake;
        self.antic.neck_up = 3.0 * raise;
        self.antic.head_pitch = 0.4 * raise;
        Phase::Stop { t, dust }
    }

    fn chase_toward(&mut self, cursor: Vec2) {
        let target = self.bounds.clamp(cursor, self.edge_margin());
        self.brain.push(Directive::RunTo(target));
    }
}
