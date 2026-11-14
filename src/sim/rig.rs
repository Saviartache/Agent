//! Процедурное тело в симуляции: вход для тела из состояния и следы от его шагов.

use std::mem;

use super::Sim;
use crate::behavior::{Decision, Facing};
use crate::math::Vec2;
use crate::rig::{Drive, Pose, RigInput, Skeleton, Vec3, World, view};

/// Сокет у кончика клюва, рыла или хобота: сюда несут карточку и предметы.
const MUZZLE: &str = "muzzle";

/// Кости тела по его позе: по ним берутся точки крепления, те же, что видны на экране.
pub(super) struct Limbs {
    drive: Drive,
    world: World,
    muzzle: Option<usize>,
}

impl Limbs {
    pub fn new(skeleton: &Skeleton, pose: &Pose) -> Self {
        let mut limbs = Self {
            drive: Drive::new(skeleton),
            world: World::default(),
            muzzle: skeleton.socket(MUZZLE),
        };
        limbs.drive.apply(skeleton, pose, &mut limbs.world);
        limbs
    }

    /// Точка на земле под сокетом морды и его высота над ней на экране, px.
    fn muzzle(&self, skeleton: &Skeleton, pos: Vec2) -> Option<(Vec2, f32)> {
        let at = skeleton.socket_point(&self.world, self.muzzle?)?;
        let (tip, floor) = (view(at), view(Vec3::new(at.x, at.y, 0.0)));
        at.is_finite()
            .then(|| (pos + Vec2::new(floor.x, floor.y), floor.y - tip.y))
    }
}

impl Sim {
    /// Кости встают по новой позе тела.
    fn pose_limbs(&mut self) {
        if let (Some(limbs), Some(rig), Some(skeleton)) =
            (&mut self.limbs, &self.rig, self.character.skeleton())
        {
            limbs.drive.apply(skeleton, rig.pose(), &mut limbs.world);
        }
    }

    /// Точка на земле под кончиком клюва, рыла или хобота и его высота над ней, px.
    /// По сокету скелета; если у вида его нет — по позе.
    pub(super) fn muzzle(&self) -> Option<(Vec2, f32)> {
        let pose = self.rig.as_ref()?.pose();
        let socket = self
            .limbs
            .as_ref()
            .zip(self.character.skeleton())
            .and_then(|(limbs, skeleton)| limbs.muzzle(skeleton, pose.pos));
        socket.or_else(|| {
            let tip = pose.muzzle_tip();
            Some((pose.screen(Vec3::on_ground(tip.ground(), 0.0)), tip.z))
        })
    }

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
        self.pose_limbs();
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
        self.pose_limbs();
    }
}
