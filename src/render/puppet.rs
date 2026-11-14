//! Тело по скелету из `rig.json`: поза водит кости, части рисуются с глубиной,
//! сверху — общие тень и мозаика.

#[cfg(test)]
mod tests;

use std::ops::Range;
use std::sync::Arc;

use super::{Canvas, Solid, creature, pixel};
use crate::math::Vec2;
use crate::rig::{Drive, Pose, Skeleton, World};

pub struct Puppet {
    skeleton: Arc<Skeleton>,
    drive: Drive,
    world: World,
    solid: Solid,
}

/// Тело персонажа на холсте. Кукла заводится один раз и меняется вместе с набором:
/// её буферы живут между кадрами.
#[derive(Default)]
pub struct Figure {
    puppet: Option<Puppet>,
}

impl Figure {
    /// `origin` — мировая точка левого верхнего угла холста.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        origin: Vec2,
        pose: &Pose,
        skeleton: &Arc<Skeleton>,
    ) {
        self.puppet(skeleton).draw(canvas, origin, pose);
    }

    /// Как `draw`, но без мозаики: её кладёт тот, кто рисует холст целиком.
    pub fn paint(
        &mut self,
        canvas: &mut Canvas,
        origin: Vec2,
        pose: &Pose,
        skeleton: &Arc<Skeleton>,
    ) {
        self.puppet(skeleton).paint(canvas, origin, pose);
    }

    fn puppet(&mut self, skeleton: &Arc<Skeleton>) -> &mut Puppet {
        if !matches!(&self.puppet, Some(puppet) if Arc::ptr_eq(&puppet.skeleton, skeleton)) {
            self.puppet = None;
        }
        self.puppet
            .get_or_insert_with(|| Puppet::new(Arc::clone(skeleton)))
    }
}

impl Puppet {
    pub fn new(skeleton: Arc<Skeleton>) -> Self {
        Self {
            drive: Drive::new(&skeleton),
            skeleton,
            world: World::default(),
            solid: Solid::default(),
        }
    }

    pub fn skeleton(&self) -> &Arc<Skeleton> {
        &self.skeleton
    }

    /// `origin` — мировая точка левого верхнего угла холста.
    pub fn draw(&mut self, canvas: &mut Canvas, origin: Vec2, pose: &Pose) {
        let drawn = self.paint(canvas, origin, pose);
        pixel::pixelate_within(canvas, origin, pose.scale, drawn);
    }

    /// Как `draw`, но без мозаики; возвращает нарисованные пиксели по `x` и `y`.
    pub fn paint(&mut self, canvas: &mut Canvas, origin: Vec2, pose: &Pose) -> [Range<usize>; 2] {
        creature::shadow(canvas, origin, pose);
        self.drive.apply(&self.skeleton, pose, &mut self.world);
        let anchor = pose.pos - origin;
        let controls = self.drive.controls();
        self.solid
            .draw(canvas, anchor, &self.skeleton, &self.world, controls)
    }
}
