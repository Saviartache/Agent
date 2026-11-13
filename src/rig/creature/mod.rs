//! Тело, которое рисуется из примитивов и живёт само — лапы шагают и встают
//! на землю, шея и голова идут с запозданием, глаза моргают. Скелет общий,
//! вид (гусь, свинка, слоник) задаёт сложение.

mod antic;
mod assemble;
mod build;
mod feet;
mod gesture;
mod head;
mod pose;
#[cfg(test)]
mod tests;

pub use self::antic::{Antic, Eyes, Look};
use self::build::Pair;
use self::feet::{Feet, Stance};
use self::gesture::{CLOCK_WRAP, Mods};
use self::head::{HeadState, Spring, wrap_angle, yaw_of};
pub use self::pose::{Body, GROUND_SQUASH, Head, NECK_POINTS, Pose, flatten};
use super::vec3::Vec3;
use super::{Gesture, RigKind};
use crate::math::Vec2;
use crate::rng::Rng;

/// Половина стороны холста вокруг точки на земле, design px: вмещает вытянутую
/// шею, прыжок на 25 px и подъём в руках.
const EXTENT: f32 = 84.0;
const MAX_DT: f32 = 0.25;
/// Внутренний шаг: короче кадра симуляции, поэтому результат не зависит от `dt`.
const SUB_SECS: f32 = 1.0 / 120.0;
const TURN_RATE: f32 = 8.0;
/// С этой скорости тело считается идущим, design px/с.
const STRIDE_SPEED: f32 = 6.0;
const MODS_RATE: f32 = 18.0;
/// Скорость бега, design px/с: от неё считается доля бега.
const RUN_SPEED: f32 = 230.0;
/// Резкие остановки не раскачивают голову сильнее, design px/с².
const ACCEL_MAX: f32 = 1200.0;
const MAX_SCALE: f32 = 64.0;

/// Что тело знает о мире на этом кадре.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigInput {
    /// Точка на земле под телом.
    pub pos: Vec2,
    pub vel: Vec2,
    pub heading: Vec2,
    pub look_at: Option<Vec2>,
    pub gesture: Gesture,
    /// Доля одноразового клипа, 0..1; у зацикленного — круг по кругу.
    pub progress: f32,
    pub scale: f32,
    /// `false`, пока тело держат за курсор.
    pub grounded: bool,
    /// Накладка сценки поверх жеста.
    pub antic: Antic,
}

/// Лапа встала на землю.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footfall {
    pub pos: Vec2,
    pub heading: Vec2,
    pub left: bool,
}

#[derive(Clone, Debug)]
pub struct Creature {
    kind: RigKind,
    scale: f32,
    pos: Vec2,
    vel: Vec2,
    yaw: f32,
    clock: f32,
    size: f32,
    grounded: bool,
    mods: Mods,
    /// Задняя пара; у птицы единственная.
    feet: Feet,
    fore: Option<Feet>,
    head: HeadState,
    /// Кончик хобота от его корня.
    trunk: Option<Spring>,
    antic: Antic,
    /// Накладка поворота, применённая к телу на прошлом кадре.
    spin: f32,
    /// Не поза, а вид — переживает `reset`.
    look: Look,
    pose: Pose,
}

impl Creature {
    pub fn new(kind: RigKind, pos: Vec2, heading: Vec2, scale: f32) -> Self {
        Self::standing(
            kind,
            clean_point(pos, Vec2::ZERO),
            clean_heading(heading, Vec2::new(1.0, 0.0)),
            clean_scale(scale, 1.0),
        )
    }

    /// Половина стороны квадратного холста вокруг `pos`, px.
    pub fn extent(scale: f32) -> f32 {
        EXTENT * scale
    }

    /// Появление, перенос, смена масштаба: лапы заново под телом, голова на месте.
    pub fn reset(&mut self, pos: Vec2, heading: Vec2, scale: f32) {
        let look = self.look;
        *self = Self::standing(
            self.kind,
            clean_point(pos, self.pos),
            clean_heading(heading, self.heading()),
            clean_scale(scale, self.scale),
        );
        self.set_look(look);
    }

    /// Следы состояний на теле: грязь, проплешина, раздутость.
    pub fn set_look(&mut self, look: Look) {
        self.look = look.cleaned();
        self.pose.look = self.look;
    }

    pub fn update(
        &mut self,
        dt: f32,
        input: &RigInput,
        rng: &mut Rng,
        footfalls: &mut Vec<Footfall>,
    ) {
        let input = self.cleaned(input);
        self.scale = input.scale;
        self.grounded = input.grounded;
        self.antic = input.antic;
        // Быстрая раскрутка не должна отставать: угол сразу в тело, сглаживание — остатку
        if input.antic.spin != 0.0 {
            self.yaw += wrap_angle(input.antic.spin - self.spin);
        }
        self.spin = input.antic.spin;
        self.size = gesture::size(input.gesture, input.progress);
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(MAX_DT)
        } else {
            0.0
        };
        let accel = if dt > 0.0 {
            ((input.vel - self.vel) * (1.0 / dt)).clamp_length(ACCEL_MAX * self.scale)
        } else {
            Vec2::ZERO
        };
        let steps = (dt / SUB_SECS).ceil().max(1.0);
        let from = self.pos;
        for i in 1..=steps as usize {
            let pos = from.lerp(input.pos, i as f32 / steps);
            self.step(dt / steps, pos, accel, &input, rng, footfalls);
        }
        self.vel = input.vel;
        self.pose = self.assemble();
    }

    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    /// Кончик клюва или рыла на экране: сюда несут карточку.
    pub fn head(&self) -> Vec2 {
        self.pose.screen(self.pose.muzzle_tip())
    }

    fn standing(kind: RigKind, pos: Vec2, heading: Vec2, scale: f32) -> Self {
        let yaw = yaw_of(heading);
        let build = kind.build();
        let stance = |pair: &Pair| pair.stance(pos, yaw, Vec2::ZERO, scale, true);
        let mut body = Self {
            kind,
            scale,
            pos,
            vel: Vec2::ZERO,
            yaw,
            clock: 0.0,
            size: 1.0,
            grounded: true,
            mods: gesture::target(Gesture::Idle, 0.0, 0.0, 0.0),
            feet: Feet::planted(&stance(&build.hind), 0),
            // Рысь: передняя пара начинает с другой ноги
            fore: build
                .fore
                .as_ref()
                .map(|pair| Feet::planted(&stance(pair), 1)),
            head: HeadState::resting(Vec3::ZERO, yaw),
            trunk: None,
            antic: Antic::default(),
            spin: 0.0,
            look: Look::default(),
            pose: Pose::default(),
        };
        body.head = HeadState::resting(body.rest(), yaw);
        body.trunk = body.trunk().map(|(_, rest)| Spring::resting(rest));
        body.pose = body.assemble();
        body
    }

    fn step(
        &mut self,
        dt: f32,
        pos: Vec2,
        accel: Vec2,
        input: &RigInput,
        rng: &mut Rng,
        footfalls: &mut Vec<Footfall>,
    ) {
        self.clock = (self.clock + dt) % CLOCK_WRAP;
        self.pos = pos;
        let build = self.kind.build();
        // Кто не ходит боком, на ходу смотрит туда, куда идёт
        let ahead = !build.sidesteps && input.vel.length() > STRIDE_SPEED * self.scale;
        let heading_yaw = yaw_of(if ahead { input.vel } else { input.heading }) + input.antic.spin;
        self.yaw += wrap_angle(heading_yaw - self.yaw) * (1.0 - (-dt * TURN_RATE).exp());
        self.yaw = wrap_angle(self.yaw);
        let run = (input.vel.length() / self.scale / RUN_SPEED).min(1.0);
        let mut target = gesture::target(input.gesture, input.progress, self.clock, run);
        target.overlay(&input.antic);
        // Кратчайшим путём: накопленные обороты не раскручиваются назад
        target.roll = self.mods.roll + wrap_angle(target.roll - self.mods.roll);
        self.mods.approach(&target, 1.0 - (-dt * MODS_RATE).exp());
        self.mods.roll = wrap_angle(self.mods.roll);
        self.feet
            .update(dt, &self.stance(&build.hind, input.vel), footfalls);
        let stance = build.fore.as_ref().map(|pair| self.stance(pair, input.vel));
        if let (Some(fore), Some(stance)) = (&mut self.fore, stance) {
            fore.update(dt, &stance, footfalls);
        }
        let aim = self.aim(accel, heading_yaw, input.look_at, run);
        self.head.update(dt, &aim, rng);
        let rest = self.trunk().map(|(_, rest)| rest);
        if let (Some(trunk), Some(rest), Some(shape)) = (&mut self.trunk, rest, &build.trunk) {
            trunk.update(dt, rest, accel, self.scale, shape.slack);
        }
    }

    fn heading(&self) -> Vec2 {
        Vec2::from_angle(self.yaw)
    }

    fn stance(&self, pair: &Pair, vel: Vec2) -> Stance {
        let grounded = self.grounded && !self.aloft();
        pair.stance(self.pos, self.yaw, vel, self.scale, grounded)
    }

    /// Мусор на входе заменяется последним хорошим значением.
    fn cleaned(&self, input: &RigInput) -> RigInput {
        RigInput {
            pos: clean_point(input.pos, self.pos),
            vel: clean_point(input.vel, Vec2::ZERO),
            heading: clean_heading(input.heading, self.heading()),
            look_at: input
                .look_at
                .filter(|at| at.x.is_finite() && at.y.is_finite()),
            gesture: input.gesture,
            progress: if input.progress.is_finite() {
                input.progress.clamp(0.0, 1.0)
            } else {
                0.0
            },
            scale: clean_scale(input.scale, self.scale),
            grounded: input.grounded,
            antic: input.antic.cleaned(),
        }
    }
}

fn clean_point(point: Vec2, fallback: Vec2) -> Vec2 {
    if point.x.is_finite() && point.y.is_finite() {
        point
    } else {
        fallback
    }
}

fn clean_heading(heading: Vec2, fallback: Vec2) -> Vec2 {
    let heading = clean_point(heading, Vec2::ZERO).normalized();
    if heading == Vec2::ZERO {
        fallback
    } else {
        heading
    }
}

fn clean_scale(scale: f32, fallback: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale.min(MAX_SCALE)
    } else {
        fallback
    }
}
