//! Шипение: гусь разворачивается к курсору, низко вытягивает шею и раскрывает крылья;
//! подойдёшь вплотную — выпад. Курсор ушёл — складывает крылья и встряхивается.

use std::f32::consts::TAU;

use super::super::Progress;
use crate::behavior::{Directive, Facing};
use crate::math::Vec2;
use crate::sim::Sim;
use crate::sim::drive::Drive;

/// Расстояния — логические px, время — с.
const NOTICE_RANGE: f32 = 250.0;
const LUNGE_RANGE: f32 = 80.0;
const END_RANGE: f32 = 300.0;
const MAX_SECS: f32 = 6.0;
const LUNGE_SPEED: f32 = 400.0;
const LUNGE_FRICTION: f32 = 10.0;
const LUNGE_SECS: f32 = 0.3;
const LUNGE_PAUSE: f32 = 1.0;
/// Разворот шажком, пока тело не встало мордой к курсору.
const TURN_SPEED: f32 = 60.0;
const TURN_LIMIT: f32 = 0.5;
const FOLD_SECS: f32 = 0.6;
const FOLD_SHAKE_HZ: f32 = 8.0;
const TREMBLE_HZ: f32 = 14.0;

#[derive(Debug)]
pub(in crate::sim) struct Hiss {
    t: f32,
    /// Сколько ещё длится выпад.
    lunge: f32,
    /// До следующего выпада.
    pause: f32,
    turn: f32,
    /// Сколько уже складывает крылья.
    fold: Option<f32>,
}

fn sign(facing: Facing) -> f32 {
    match facing {
        Facing::Left => -1.0,
        Facing::Right => 1.0,
    }
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_hiss(&mut self) -> Option<Hiss> {
        let cursor = self.cursor?;
        if cursor.distance(self.body.pos) > NOTICE_RANGE * self.ui {
            return None;
        }
        Some(Hiss {
            t: 0.0,
            lunge: 0.0,
            pause: 0.0,
            turn: 0.0,
            fold: None,
        })
    }

    pub(in crate::sim::stunt) fn step_hiss(&mut self, mut act: Hiss, dt: f32) -> Progress<Hiss> {
        act.t += dt;
        let cursor = self
            .cursor
            .filter(|c| act.fold.is_some() || c.distance(self.body.pos) <= END_RANGE * self.ui);
        if act.fold.is_none() && (cursor.is_none() || act.t > MAX_SECS) {
            act.fold = Some(0.0);
            self.drive = Some(Drive::new(Vec2::ZERO));
        }
        if let Some(fold) = act.fold {
            return self.fold(act, fold + dt);
        }
        let to = cursor.map_or(Vec2::ZERO, |c| c - self.body.pos);
        let dist = to.length();
        act.pause -= dt;
        if act.lunge > 0.0 {
            act.lunge -= dt;
        } else if dist < LUNGE_RANGE * self.ui && act.pause <= 0.0 {
            let vel = to.normalized() * (LUNGE_SPEED * self.ui);
            self.drive = Some(Drive::new(vel).friction(LUNGE_FRICTION));
            act.lunge = LUNGE_SECS;
            act.pause = LUNGE_PAUSE;
        } else {
            self.drive = Some(Drive::new(self.turning(&mut act, to, dt)));
        }
        let a = &mut self.antic;
        a.neck_up = -8.0;
        a.neck_forward = 6.0;
        a.head_pitch = -0.2;
        a.wings = 1.0;
        a.mouth = if act.lunge > 0.0 { 1.0 } else { 0.6 };
        a.wobble = 0.6 * (TAU * TREMBLE_HZ * act.t).sin();
        Progress::Going(act)
    }

    /// Шажок в сторону курсора, пока клюв смотрит не туда.
    fn turning(&mut self, act: &mut Hiss, to: Vec2, dt: f32) -> Vec2 {
        let want = if to.x < 0.0 { -1.0 } else { 1.0 };
        if want == sign(self.facing) {
            act.turn = 0.0;
            return Vec2::ZERO;
        }
        if act.turn >= TURN_LIMIT {
            return Vec2::ZERO;
        }
        if act.turn <= 0.0 {
            let ahead = self.body.pos + Vec2::new(want * 30.0 * self.ui, 0.0);
            self.brain.push(Directive::MoveTo(ahead));
        }
        act.turn = (act.turn + dt).min(TURN_LIMIT);
        Vec2::new(want * TURN_SPEED * self.ui, 0.0)
    }

    fn fold(&mut self, mut act: Hiss, fold: f32) -> Progress<Hiss> {
        if fold >= FOLD_SECS {
            self.drive = None;
            self.halt();
            return Progress::Finished;
        }
        act.fold = Some(fold);
        let k = fold / FOLD_SECS;
        self.antic.wings = 1.0 - k;
        self.antic.wobble = 2.0 * (1.0 - k) * (TAU * FOLD_SHAKE_HZ * fold).sin();
        Progress::Going(act)
    }
}
