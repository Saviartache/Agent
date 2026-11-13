//! Мяч: появляется рядом, персонаж обегает его, чтобы бить в сторону цели, и пинает
//! трижды. Курсор, задевший мяч на ходу, толкает его сам. После последнего пинка — радость.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::effects::props::{Prop, PropId, PropKind};
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, скорости — px/с, время — с.
const TIMEOUT: f32 = 22.0;
const BALL_DISTANCE: [f32; 2] = [110.0, 130.0];
const KICKS: u8 = 3;
const KICK_SPEED: [f32; 2] = [400.0, 600.0];
const KICK_LOFT: f32 = 150.0;
/// Мяч живёт столько после последнего пинка.
const LAST_LIFE: f32 = 10.0;
const BALL_TTL: f32 = 40.0;
/// Бьёт, стоя на таком расстоянии позади мяча.
const BEHIND: f32 = 32.0;
/// Допуск до точки удара.
const STRIKE_REACH: f32 = 24.0;
/// Цель вдоль стены для мяча, застрявшего у края.
const ALONG: f32 = 300.0;
/// Обегает мяч на таком расстоянии сбоку.
const AROUND: f32 = 60.0;
/// Покатившийся мяч быстрее этого не догоняют, пока не замедлится.
const SETTLED_SPEED: f32 = 200.0;
/// Курсор задевает мяч в столько его радиусов и должен двигаться быстрее этого.
const TOUCH_REACH: f32 = 1.5;
const TOUCH_SPEED: f32 = 80.0;
const TOUCH_SHARE: f32 = 0.5;
const LUNGE_SECS: f32 = 0.3;
const CHEER_SECS: f32 = 1.2;

#[derive(Debug)]
pub(in crate::sim) struct Ball {
    ball: PropId,
    /// Куда бьёт сейчас.
    goal: Vec2,
    kicks: u8,
    lunge: f32,
    /// Доигрывает после последнего пинка.
    cheer: Option<f32>,
    left: f32,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_ball(&mut self) -> Option<Ball> {
        let at = self.spot_away(BALL_DISTANCE, BALL_DISTANCE[0] * 0.8, 50.0)?;
        let ball = self.props.spawn(PropKind::Ball, at, 0.0, self.ui, BALL_TTL);
        Some(Ball {
            ball,
            goal: self.new_goal(),
            kicks: 0,
            lunge: 0.0,
            cheer: None,
            left: TIMEOUT,
        })
    }

    pub(in crate::sim::stunt) fn step_ball(&mut self, mut act: Ball, dt: f32) -> Progress<Ball> {
        act.left -= dt;
        let Some(ball) = self.props.get(act.ball).copied() else {
            return Progress::Dropped;
        };
        if act.left <= 0.0 {
            return Progress::Dropped;
        }
        self.push_ball(&ball);
        if let Some(left) = act.cheer {
            return match left > dt {
                true => {
                    act.cheer = Some(left - dt);
                    Progress::Going(act)
                }
                false => Progress::Finished,
            };
        }
        act.lunge = (act.lunge - dt).max(0.0);
        self.antic.neck_forward = act.lunge / LUNGE_SECS;
        let margin = self.edge_margin();
        let inner = self.bounds.clamp(ball.pos, margin);
        if inner != ball.pos {
            // Мяч у самой стены, куда не встать: пинает вдоль неё
            let out = (ball.pos - inner).normalized();
            let along = Vec2::new(-out.y, out.x);
            let center = self.bounds.nearest(ball.pos).center();
            let along = if along.dot(center - ball.pos) < 0.0 {
                -along
            } else {
                along
            };
            // В углу вдоль стены некуда: бьёт к середине
            let corner = out.x.abs() > 0.1 && out.y.abs() > 0.1;
            act.goal = if corner {
                center
            } else {
                ball.pos + along * (ALONG * self.ui)
            };
        }
        let to_goal = (act.goal - ball.pos).normalized();
        let behind = ball.pos - to_goal * (BEHIND * self.ui);
        let strike = self.bounds.clamp(behind, margin);
        // Стоит перед мячом — обходит; сбоку уже можно идти к точке удара.
        // Встать позади мяча негде — бьёт оттуда, где может стоять
        let offside =
            strike == behind && (self.body.pos - ball.pos).dot(to_goal) > 0.3 * BEHIND * self.ui;
        let rolling = ball.vel.length() > SETTLED_SPEED * self.ui;
        let aim = if offside {
            // Мяч между телом и целью: обегает сбоку
            let side = to_goal.x * (self.body.pos.y - ball.pos.y)
                - to_goal.y * (self.body.pos.x - ball.pos.x);
            let perp = Vec2::new(-to_goal.y, to_goal.x) * side.signum();
            ball.pos + perp * (AROUND * self.ui)
        } else {
            strike
        };
        self.brain
            .push(Directive::RunTo(self.bounds.clamp(aim, self.edge_margin())));
        if !offside && !rolling && self.body.pos.distance(strike) <= STRIKE_REACH * self.ui {
            self.kick(&mut act, to_goal);
        }
        Progress::Going(act)
    }

    fn kick(&mut self, act: &mut Ball, to_goal: Vec2) {
        let speed = self.rng.range(KICK_SPEED[0], KICK_SPEED[1]) * self.ui;
        let loft = KICK_LOFT * self.ui;
        let at = self.props.get_mut(act.ball).map(|ball| {
            ball.vel = to_goal * speed;
            ball.vz = loft;
            ball.pos
        });
        self.burst(Burst::Dust, at.unwrap_or(self.body.pos));
        act.lunge = LUNGE_SECS;
        act.kicks += 1;
        act.goal = self.new_goal();
        if act.kicks >= KICKS {
            if let Some(ball) = self.props.get_mut(act.ball) {
                ball.expire_in(LAST_LIFE);
            }
            self.halt();
            self.perform_quietly(self.character.actions().celebrate);
            act.cheer = Some(CHEER_SECS);
        }
    }

    fn new_goal(&mut self) -> Vec2 {
        let area = self.bounds.nearest(self.body.pos);
        let at = Vec2::new(
            self.rng.range(area.min.x, area.max.x),
            self.rng.range(area.min.y, area.max.y),
        );
        area.clamp(at, self.edge_margin())
    }

    /// Курсор, задевший мяч на ходу, толкает его.
    fn push_ball(&mut self, ball: &Prop) {
        let Some(cursor) = self.cursor else {
            return;
        };
        let touch = cursor.distance(ball.pos) <= ball.radius * TOUCH_REACH;
        if touch && self.cursor_vel.length() > TOUCH_SPEED * self.ui {
            let push = self.cursor_vel * TOUCH_SHARE;
            if let Some(ball) = self.props.get_mut(ball.id()) {
                ball.vel = push;
            }
        }
    }
}
