//! Следы, частицы и события мозга: что остаётся от движения и реакций.

use std::mem;

use super::{HALF_GAUGE, MOVING_SPEED, Sim, push_event};
use crate::action::ActionId;
use crate::behavior::{BrainEvent, Reaction};
use crate::command::Stunt;
use crate::effects::particles::Burst;
use crate::event::AgentEvent;
use crate::math::Vec2;

fn burst_for(reaction: Reaction) -> Burst {
    match reaction {
        Reaction::Click | Reaction::Contact => Burst::Sparks,
        Reaction::Startle => Burst::Dust,
        Reaction::Joy => Burst::Confetti,
        Reaction::Alarm => Burst::Alert,
        Reaction::Failure => Burst::Smoke,
    }
}

impl Sim {
    pub(super) fn age_effects(&mut self, dt: f32) {
        // У процедурного тела следы оставляют его шаги
        if self.settings.footprints && self.rig.is_none() {
            let moving = self.body.vel.length() > MOVING_SPEED * self.ui;
            let grounded = self.alive() && !self.dragging() && moving;
            self.footprints.track(
                self.body.pos,
                self.body.heading,
                HALF_GAUGE * self.ui,
                grounded,
            );
        }
        self.footprints.update(dt);
        self.mud_prints.update(dt);
        self.dry_off(dt);
        self.particles.update(dt);
        self.leaves.update(dt);
        self.mud.update(dt);
        let area = self.bounds.nearest(self.body.pos);
        self.props.update(dt, area, self.ui);
        self.scribble.update(dt);
        self.tend_conditions(dt);
        self.collect_closed(dt);
    }

    pub(super) fn burst(&mut self, kind: Burst, at: Vec2) {
        if self.settings.particles {
            self.particles.burst(kind, at, self.ui, &mut self.rng);
        }
    }

    /// События мозга уходят приложению, в частицы и в сценки.
    pub(super) fn react(&mut self) {
        let mut events = mem::take(&mut self.brain_events);
        for event in events.drain(..) {
            match event {
                BrainEvent::Spawned => push_event(&mut self.events, AgentEvent::Spawned),
                BrainEvent::Despawned => {
                    self.press = None;
                    push_event(&mut self.events, AgentEvent::Despawned);
                }
                BrainEvent::Reacted(reaction) => self.burst(burst_for(reaction), self.head()),
                BrainEvent::Performed(action) => self.announce(action),
                // Догнал курсор: может и утащить, если разрешено
                BrainEvent::WantsCursorGrab => {
                    self.stage(Stunt::CursorHeist);
                }
                BrainEvent::IntentChanged(_) | BrainEvent::StatusSettled => {}
            }
        }
        self.brain_events = events;
    }

    fn announce(&mut self, action: ActionId) {
        if self.silent == Some(action) {
            self.silent = None;
            return;
        }
        let name = self.character.action(action).name.clone();
        push_event(&mut self.events, AgentEvent::Played(name));
    }
}
