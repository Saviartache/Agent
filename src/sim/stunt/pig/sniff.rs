//! Нюх: идёт по запаху — по пути, который курсор прошёл недавно.

use std::f32::consts::{PI, TAU};

use super::super::Progress;
use crate::behavior::Directive;
use crate::effects::particles::Burst;
use crate::rig::Eyes;
use crate::sim::Sim;

/// Расстояния — логические px, время — с, частоты — Гц.
const SCENT_AGO: f32 = 3.0;
const AIR_SECS: f32 = 1.0;
const NOD_HZ: f32 = 4.0;
const RETARGET_GAP: f32 = 0.3;
const DUST_GAP: f32 = 0.5;
const CAUGHT: f32 = 40.0;
const TRACK_SECS: f32 = 15.0;
const SNORT_SECS: f32 = 0.6;
const SNORT_HOP: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Air(f32),
    Track { left: f32, retarget: f32, dust: f32 },
    Snort(f32),
}

#[derive(Debug)]
pub(in crate::sim) struct Sniff {
    phase: Phase,
}

impl Sim {
    pub(in crate::sim::stunt) fn begin_sniff(&mut self) -> Option<Sniff> {
        self.trail.ago(SCENT_AGO)?;
        self.halt();
        Some(Sniff {
            phase: Phase::Air(AIR_SECS),
        })
    }

    pub(in crate::sim::stunt) fn step_sniff(&mut self, act: Sniff, dt: f32) -> Progress<Sniff> {
        let phase = match act.phase {
            Phase::Air(left) => {
                self.antic.pitch = 0.3;
                self.antic.neck_up = 4.0;
                self.antic.head_pitch = 0.5 + 0.15 * (TAU * NOD_HZ * (AIR_SECS - left)).sin();
                if left > dt {
                    Phase::Air(left - dt)
                } else {
                    Phase::Track {
                        left: TRACK_SECS,
                        retarget: 0.0,
                        dust: DUST_GAP,
                    }
                }
            }
            Phase::Track {
                left,
                retarget,
                dust,
            } => {
                // Курсор пропал — запах оборвался
                let Some(scent) = self.trail.ago(SCENT_AGO) else {
                    return Progress::Dropped;
                };
                if left <= dt {
                    return Progress::Dropped;
                }
                if self
                    .cursor
                    .is_some_and(|c| c.distance(self.body.pos) <= CAUGHT * self.ui)
                {
                    self.halt();
                    self.burst(Burst::Puff, self.head());
                    return Progress::Going(Sniff {
                        phase: Phase::Snort(SNORT_SECS),
                    });
                }
                self.antic.neck_forward = 4.0;
                self.antic.head_pitch = -0.5;
                let retarget = if retarget <= dt {
                    self.brain.push(Directive::MoveTo(scent));
                    RETARGET_GAP
                } else {
                    retarget - dt
                };
                let dust = if dust <= dt {
                    self.burst(Burst::Dust, self.head());
                    DUST_GAP
                } else {
                    dust - dt
                };
                Phase::Track {
                    left: left - dt,
                    retarget,
                    dust,
                }
            }
            Phase::Snort(left) => {
                let u = 1.0 - left / SNORT_SECS;
                self.antic.eyes = Eyes::Wide;
                self.antic.lift = SNORT_HOP * (PI * u).sin().max(0.0);
                if left <= dt {
                    return Progress::Finished;
                }
                Phase::Snort(left - dt)
            }
        };
        Progress::Going(Sniff { phase })
    }
}
