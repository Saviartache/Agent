//! Лужа грязи: разливается поодаль, свинка бежит к ней, валяется с брызгами и уходит
//! вымазанной — грязь видна на спине и копытах, а следы тянутся жирной бурой дорожкой,
//! которая бледнеет шаг за шагом, пока грязь не сотрётся.

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::math::Vec2;
use crate::sim::Sim;

/// Расстояния — логические px, время — с.
const PUDDLE_RADIUS: f32 = 34.0;
/// Нетронутая лужа сохнет сама.
const PUDDLE_TTL: f32 = 18.0;
const PUDDLE_DISTANCE: [f32; 2] = [220.0, 420.0];
const PUDDLE_MIN_DISTANCE: f32 = 120.0;
const NOTICE_SECS: f32 = 0.8;
/// Не добежала за это время — сценка сорвалась.
const TROT_SECS: f32 = 12.0;
/// Ближе этой доли радиуса к середине — уже в луже.
const ENTER: f32 = 0.6;
const WALLOW_SECS: f32 = 1.6;
const SPLASH_GAP: f32 = 0.4;
/// Грязи хватает на столько шагов: дорожка длинная и бледнеет к концу.
const MUDDY_STEPS: f32 = 90.0;
/// Стоящая свинка обсыхает и без шагов.
const DRY_SECS: f32 = 90.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Notice(f32),
    Trot(f32),
    Wallow { left: f32, splash: f32 },
}

#[derive(Clone, Copy, Debug)]
pub(in crate::sim) struct MudBath {
    puddle: Vec2,
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_mud_bath(&mut self) -> Option<MudBath> {
        let puddle = self.spot_away(PUDDLE_DISTANCE, PUDDLE_MIN_DISTANCE, PUDDLE_RADIUS)?;
        self.mud
            .pour(puddle, PUDDLE_RADIUS * self.ui, PUDDLE_TTL, &mut self.rng);
        Some(MudBath {
            puddle,
            phase: Phase::Notice(NOTICE_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_mud_bath(
        &mut self,
        act: MudBath,
        dt: f32,
    ) -> Progress<MudBath> {
        let phase = match act.phase {
            Phase::Notice(left) if left > dt => Phase::Notice(left - dt),
            Phase::Notice(_) => {
                self.brain.push(Directive::RunTo(act.puddle));
                Phase::Trot(TROT_SECS)
            }
            Phase::Trot(_)
                if self.body.pos.distance(act.puddle) <= ENTER * PUDDLE_RADIUS * self.ui =>
            {
                self.halt();
                self.burst(Burst::Mud, self.body.pos);
                self.perform_quietly(self.character.actions().celebrate);
                Phase::Wallow {
                    left: WALLOW_SECS,
                    splash: SPLASH_GAP,
                }
            }
            Phase::Trot(left) if left > dt => Phase::Trot(left - dt),
            Phase::Trot(_) => return Progress::Dropped,
            Phase::Wallow { left, splash } => {
                let left = left - dt;
                // Вымазывается постепенно, пока валяется
                self.cond.muddy = self.cond.muddy.max(1.0 - left.max(0.0) / WALLOW_SECS);
                if left <= 0.0 {
                    self.cond.muddy = 1.0;
                    self.mud.dry_at(act.puddle);
                    return Progress::Finished;
                }
                let splash = if splash <= dt {
                    self.burst(Burst::Mud, self.body.pos);
                    SPLASH_GAP
                } else {
                    splash - dt
                };
                Phase::Wallow { left, splash }
            }
        };
        Progress::Going(MudBath { phase, ..act })
    }

    /// Шаг вымазанной свинки: грязный след вместо обычного, жирный по остатку грязи.
    /// `false` — грязи нет, шаг обычный.
    pub(in crate::sim) fn muddy_step(
        &mut self,
        at: Vec2,
        heading: Vec2,
        left: bool,
        clear: f32,
    ) -> bool {
        if self.cond.muddy <= 0.0 {
            return false;
        }
        self.mud_prints
            .stamp(at, heading, left, clear, self.cond.muddy);
        self.cond.muddy = (self.cond.muddy - 1.0 / MUDDY_STEPS).max(0.0);
        true
    }

    pub(in crate::sim) fn dry_off(&mut self, dt: f32) {
        self.cond.muddy = (self.cond.muddy - dt / DRY_SECS).max(0.0);
    }
}
