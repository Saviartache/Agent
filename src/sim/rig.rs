//! Процедурное тело в симуляции: вход для тела из состояния и следы от его шагов.

use std::mem;

use super::Sim;
use crate::behavior::{Decision, Facing};
use crate::math::Vec2;
use crate::rig::{RigInput, Vec3};

impl Sim {
    /// Тело по ходу движения, но не против стороны, которую выбрал мозг.
    fn rig_heading(&self) -> Vec2 {
        let side = match self.facing {
            Facing::Left => -1.0,
            Facing::Right => 1.0,
        };
        if self.body.heading.x * side < 0.0 {
            Vec2::new(side, 0.0)
        } else {
            self.body.heading
        }
    }

    /// Появление, перенос и смена масштаба: лапы заново под телом.
    pub(super) fn reset_rig(&mut self) {
        let heading = self.rig_heading();
        if let Some(rig) = &mut self.rig {
            rig.reset(self.body.pos, heading, self.sprite);
        }
    }

    pub(super) fn animate_rig(&mut self, decision: &Decision, dt: f32) {
        let heading = self.rig_heading();
        let (gesture, progress) = self.anim.gesture(&self.character);
        let grounded = !self.dragging();
        let Some(rig) = &mut self.rig else {
            return;
        };
        rig.set_look(self.cond.look());
        let input = RigInput {
            pos: self.body.pos,
            vel: self.body.vel,
            heading,
            look_at: decision.look_at,
            gesture,
            progress,
            scale: self.sprite,
            grounded,
            antic: self.antic,
        };
        rig.update(dt, &input, &mut self.rng, &mut self.footfalls);
        let pose = rig.pose();
        // Четыре ноги ставят следы тесно: каждому — место в свой диаметр
        let clear = match pose.fore {
            Some(_) => 2.0 * pose.kind.print_radius() * self.sprite,
            None => 0.0,
        };
        // След остаётся там, где лапка нарисована, а не где она стоит в мире
        for step in &mut self.footfalls {
            step.pos = pose.screen(Vec3::on_ground(step.pos, 0.0));
        }
        let mut footfalls = mem::take(&mut self.footfalls);
        for step in footfalls.drain(..) {
            if self.settings.footprints
                && !self.muddy_step(step.pos, step.heading, step.left, clear)
            {
                self.footprints
                    .stamp(step.pos, step.heading, step.left, clear, 1.0);
            }
        }
        // Буфер шагов живёт дальше: без выделения памяти на каждый кадр
        self.footfalls = footfalls;
    }
}
