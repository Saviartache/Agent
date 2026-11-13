//! Сборка позы: из состояния ног, головы и поправок жеста — точки тела в мире.

use std::f32::consts::{FRAC_PI_2, TAU};

use super::Creature;
use super::build::Build;
use super::feet::Feet;
use super::head::Aim;
use super::pose::{Body, Head, NECK_POINTS, Pose};
use crate::math::Vec2;
use crate::rig::vec3::{Frame, Vec3};

// Размеры — design px (умножаются на масштаб), углы — радианы.
/// Тело приподнимается на долю высоты шагающей лапы и качается в сторону опорной.
const BOB: f32 = 0.35;
const SWAY: f32 = 0.4;
/// Насколько тело поднято, пока его держат за курсор.
const DRAG_LIFT: f32 = 10.0;
const BREATH_HZ: f32 = 0.4;
/// Голова ходит вперёд-назад на столько px, когда лапа поднята на `HEAD_BOB_LIFT`.
const HEAD_BOB: f32 = 2.5;
const HEAD_BOB_LIFT: f32 = 3.5;
/// Изгиб поднятой шеи: контрольная точка выше и позади основания, в длинах шеи.
const NECK_ARCH_UP: f32 = 0.75;
const NECK_ARCH_BACK: f32 = 0.2;
/// Опущенная шея — дуга вперёд: контрольная точка на этой доле пути к голове по земле
/// и чуть выше основания.
const NECK_DROOP_REACH: f32 = 0.55;
const NECK_DROOP_UP: f32 = 0.25;
/// Шея считается поднятой, когда голова выше основания на эту долю её длины.
const NECK_RAISED: f32 = 0.6;
/// Опускаясь вперёд, шея не короче этой доли длины в покое: голова идёт дугой.
const NECK_KEEP: f32 = 0.9;
const DANGLE_SWAY: f32 = 2.5;
const DANGLE_HZ: f32 = 2.0;
/// Тень слабеет вдвое на этой высоте, design px.
const SHADOW_HALF: f32 = 10.0;
/// Раздутое тело: прибавка к полуосям и провисание брюха, design px.
const BLOAT_GROW: f32 = 0.35;
const BLOAT_SAG: f32 = 1.0;
/// Лапы висят свободно, когда тело выше этого, design px, и когда завалено дальше этого.
const AIR_HOP: f32 = 1.5;
const AIR_LYING: f32 = 0.05;
/// Лапы под повёрнутым телом торчат на столько ниже его низа, design px.
const LEG_PROTRUDE: f32 = 2.5;
/// Голова лежащего тела: дальше основания шеи вперёд, запрокидывается на спине.
const LYING_REACH: f32 = 4.0;
const LYING_HEAD_PITCH: f32 = 0.6;
/// Тело не сплющить в ноль и не вывернуть наизнанку.
const MIN_RATIO: f32 = 0.1;
/// Корень хобота на морде: вперёд и вверх в радиусах головы.
const TRUNK_ROOT: [f32; 2] = [0.8, -0.3];
/// Поднятый хобот загибается назад не дальше этого наклона, рад.
const TRUNK_RAISE: f32 = 2.2;
/// Кончик хобота не ниже этого над землёй, design px: опущенный ложится вперёд.
const TRUNK_FLOOR: f32 = 1.5;

fn cycle(clock: f32, hz: f32) -> f32 {
    (TAU * hz * clock).sin()
}

impl Creature {
    fn build(&self) -> &'static Build {
        self.kind.build()
    }

    /// Наклон стойки: завалившись на бок или спину, тело ложится вдоль земли.
    fn stand_pitch(&self) -> f32 {
        self.build().body_pitch * self.mods.roll.cos().max(0.0)
    }

    fn body_pitch(&self) -> f32 {
        self.stand_pitch() + self.mods.pitch
    }

    fn body_frame(&self) -> Frame {
        Frame::new(self.yaw, self.body_pitch(), self.mods.roll).rocked(self.yaw, self.rock())
    }

    /// Оси, вдоль которых висят лапы: у тела торчком — отвесно вниз, а не к спине.
    fn legs_frame(&self) -> Frame {
        let pitch = self.body_pitch() - self.stand_pitch() * self.upright();
        Frame::new(self.yaw, pitch, self.mods.roll).rocked(self.yaw, self.rock())
    }

    /// Какая лапа поднята, −1…1: плюс — левая.
    fn stride(&self) -> f32 {
        let lift = (self.feet.lift(0) - self.feet.lift(1)) / (HEAD_BOB_LIFT * self.scale);
        lift.clamp(-1.0, 1.0)
    }

    /// Вразвалку: тело клонится к опорной лапе.
    fn rock(&self) -> f32 {
        self.build().waddle * self.stride()
    }

    fn right(&self) -> Vec2 {
        let forward = self.heading();
        Vec2::new(-forward.y, forward.x)
    }

    /// Во сколько раз вытянуты полуоси: сжатие сценки и раздутость.
    fn ratio(&self) -> [f32; 3] {
        let m = &self.mods;
        let grow = 1.0 + BLOAT_GROW * self.look.bloat;
        let wide = (1.0 + m.stretch_x).max(MIN_RATIO) * grow;
        let tall = (1.0 + m.stretch_y).max(MIN_RATIO) * grow;
        // Рост меняется вдоль той оси, что стоит торчком
        let upright = self.upright();
        let lerp = |a: f32, b: f32| a + (b - a) * upright;
        [lerp(wide, tall), wide, lerp(tall, wide)]
    }

    /// Насколько продольная ось тела стоит торчком, 0…1: до 30° — лежит, от 60° — стоит.
    fn upright(&self) -> f32 {
        ((self.stand_pitch().sin().powi(2) - 0.25) * 2.0).clamp(0.0, 1.0)
    }

    /// Полуось по росту.
    fn tall(&self, radii: [f32; 3]) -> f32 {
        radii[2] + (radii[0] - radii[2]) * self.upright()
    }

    fn radii(&self) -> [f32; 3] {
        let (ratio, radii) = (self.ratio(), self.build().body_radii);
        std::array::from_fn(|i| radii[i] * ratio[i])
    }

    /// Завален ли корпус: 0 стоит, 2 на спине.
    fn lying(&self) -> f32 {
        1.0 - self.mods.roll.cos()
    }

    /// Тело оторвано от земли или завалено: лапы не шагают.
    pub(super) fn aloft(&self) -> bool {
        self.mods.hop > AIR_HOP || self.lying() > AIR_LYING
    }

    /// Доля свободных лап: в руках, в прыжке и на боку они висят у тела.
    fn loose(&self) -> f32 {
        if !self.grounded {
            return 1.0;
        }
        // Мёртвая зона: затухающий хвост жеста не сдвигает лапы
        let ramp = |v: f32, from: f32, to: f32| ((v - from) / (to - from)).clamp(0.0, 1.0);
        ramp(self.lying(), 0.01, AIR_LYING).max(ramp(self.mods.hop, 0.5, AIR_HOP))
    }

    fn body_center(&self) -> Vec3 {
        let (left, right) = (self.feet.lift(0), self.feet.lift(1));
        let m = &self.mods;
        let drag = if self.grounded {
            0.0
        } else {
            DRAG_LIFT * self.scale
        };
        let build = self.build();
        let radii = self.radii();
        let (side, high) = (radii[1], self.tall(radii));
        // Низ тела на месте при любом сжатии; брюхо раздутого провисает
        let stand =
            build.stand_z - self.tall(build.body_radii) + high - BLOAT_SAG * self.look.bloat;
        let (sin, cos) = m.roll.sin_cos();
        // Завалившись, тело ложится на землю своей шириной
        let rest = (side * sin).hypot(high * cos);
        let height = rest + (stand - high) * cos.max(0.0);
        let z = (height - m.crouch + m.hop + m.breath * cycle(self.clock, BREATH_HZ)) * self.scale
            + left.max(right) * BOB
            + drag;
        let aside = (left - right) * SWAY + m.wobble * self.scale;
        Vec3::on_ground(self.pos + self.right() * aside, z)
    }

    fn neck_base(&self, frame: &Frame, center: Vec3) -> Vec3 {
        let (s, base, ratio) = (self.scale, self.build().neck_base, self.ratio());
        frame.point(center, base[0] * ratio[0] * s, 0.0, base[1] * ratio[2] * s)
    }

    /// Куда голова тянется от основания шеи.
    pub(super) fn rest(&self) -> Vec3 {
        let (s, m, build) = (self.scale, &self.mods, self.build());
        let (frame, center) = (self.body_frame(), self.body_center());
        let (forward, right) = (self.heading(), self.right());
        // Голова клюёт вперёд в такт шагу: лапы по очереди
        let slack = build.neck_slack;
        let reach = build.neck_rest[0] + (m.neck_forward + HEAD_BOB * self.stride()) * slack;
        let (reach, up) = keep_length(reach, build.neck_rest[1] + m.neck_up * slack, build);
        let aside = m.neck_side * slack;
        let upright = Vec3::new(
            forward.x * reach + right.x * aside,
            forward.y * reach + right.y * aside,
            up,
        ) * s;
        let base = self.neck_base(&frame, center);
        // Лёжа голова на земле впереди груди
        let on_ground = Vec3::new(
            forward.x * (build.neck_rest[0] + LYING_REACH),
            forward.y * (build.neck_rest[0] + LYING_REACH),
            0.0,
        ) * s;
        let lying = Vec3::new(on_ground.x, on_ground.y, build.head_radius * s - base.z);
        let upright = upright.lerp(lying, self.rolled());
        let curl = build.curl_rest.map(|c| c * s);
        let curled = frame.point(center, curl[0], curl[1], curl[2]);
        upright.lerp(curled - base, m.curl)
    }

    /// Насколько тело завалено, 0…1.
    fn rolled(&self) -> f32 {
        (self.lying() * 4.0).clamp(0.0, 1.0)
    }

    /// На спине голова запрокинута.
    fn head_pitch(&self) -> f32 {
        let m = &self.mods;
        self.build().head_pitch + m.head_pitch + LYING_HEAD_PITCH * (-m.roll.cos()).max(0.0)
    }

    /// Корень хобота и куда от него тянется кончик; голова кивает — хобот
    /// поднимается или опускается сильнее неё.
    pub(super) fn trunk(&self) -> Option<(Vec3, Vec3)> {
        let trunk = self.build().trunk.as_ref()?;
        let (s, m) = (self.scale, &self.mods);
        let center = self
            .head
            .center(self.neck_base(&self.body_frame(), self.body_center()));
        let (yaw, pitch) = (self.head.yaw(), self.head_pitch());
        let head = Frame::new(yaw, pitch, m.head_roll + m.roll);
        let radius = self.build().head_radius * s;
        let root = head.point(center, TRUNK_ROOT[0] * radius, 0.0, TRUNK_ROOT[1] * radius);
        let bend = (trunk.droop + trunk.gain * pitch).clamp(-FRAC_PI_2, TRUNK_RAISE);
        let length = trunk.length * s;
        let reach = Frame::new(yaw, bend, 0.0).forward * length;
        let drop = root.z - TRUNK_FLOOR * s;
        if root.z + reach.z >= TRUNK_FLOOR * s {
            return Some((root, reach));
        }
        // Не достаёт до земли сквозь неё: ложится вперёд
        let flat = (length * length - drop.max(0.0).powi(2)).max(0.0).sqrt();
        let ahead = Vec2::from_angle(yaw) * flat;
        Some((root, Vec3::new(ahead.x, ahead.y, -drop)))
    }

    pub(super) fn aim(
        &self,
        accel: Vec2,
        heading_yaw: f32,
        look_at: Option<Vec2>,
        run: f32,
    ) -> Aim {
        let (frame, center) = (self.body_frame(), self.body_center());
        Aim {
            base: self.neck_base(&frame, center),
            rest: self.rest(),
            accel,
            body_yaw: self.yaw,
            heading_yaw,
            look_at,
            yaw_offset: self.mods.head_yaw,
            calm: 1.0 - run,
            scale: self.scale,
            slack: self.build().neck_slack,
            yaw_limit: self.build().head_turn,
        }
    }

    pub(super) fn assemble(&self) -> Pose {
        let (s, m, build) = (self.scale, &self.mods, self.build());
        let (frame, center) = (self.body_frame(), self.body_center());
        let base = self.neck_base(&frame, center);
        let head_center = self.head.center(base);
        let lift = m.hop.max(0.0) + if self.grounded { 0.0 } else { DRAG_LIFT };
        let legs = self.legs_frame();
        let pair =
            |feet: &Feet, along| [0, 1].map(|leg| self.foot(feet, along, &legs, center, leg));
        let trunk = self
            .trunk()
            .zip(self.trunk.as_ref())
            .map(|((root, _), tip)| [root, root + tip.rel()]);
        let rock = self.rock();
        let mut antic = self.antic;
        if build.gesture_wings {
            antic.wings = antic.wings.max(m.wings);
        }
        let mut pose = Pose {
            kind: self.kind,
            pos: self.pos,
            scale: s,
            body: Body {
                center,
                forward: self.heading(),
                pitch: self.body_pitch(),
                roll: m.roll,
                rock,
                radii: self.radii().map(|r| r * s),
            },
            neck: neck(base, head_center, self.heading(), 1.0 - self.rolled()),
            head: Head {
                center: head_center,
                dir: Vec2::from_angle(self.head.yaw()),
                pitch: self.head_pitch(),
                // Крен головы плюсом клонит макушку влево, развалка — вправо
                roll: m.head_roll + m.roll - rock,
                radius: build.head_radius * s,
                muzzle: build.muzzle * s,
                eyes_open: (1.0 - m.lids).clamp(0.0, 1.0) * self.head.eyes_open(),
            },
            feet: pair(&self.feet, build.hind.along),
            fore: self
                .fore
                .as_ref()
                .zip(build.fore.as_ref())
                .map(|(feet, fore)| pair(feet, fore.along)),
            trunk,
            tail: self.tail.lag(),
            shadow: 1.0 / (1.0 + lift / SHADOW_HALF),
            tuck: m.tuck.clamp(0.0, 1.0),
            antic,
            look: self.look,
        };
        if self.size != 1.0 {
            pose.zoomed(self.size);
        }
        pose
    }

    /// Лапа пары, отнесённой на `along` по ходу; в руках она висит под телом.
    fn foot(&self, feet: &Feet, along: f32, frame: &Frame, center: Vec3, leg: usize) -> Vec3 {
        let planted = Vec3::on_ground(feet.ground(leg), feet.lift(leg));
        let loose = self.loose();
        if loose <= 0.0 {
            return planted;
        }
        let (s, build, ratio) = (self.scale, self.build(), self.ratio());
        let side = if leg == 0 { -1.0 } else { 1.0 };
        let swing = DANGLE_SWAY * cycle(self.clock + 0.15 * leg as f32, DANGLE_HZ);
        // Пары висят поровну по обе стороны от центра тела
        let hind = build.hind.along;
        let middle = build
            .fore
            .as_ref()
            .map_or(hind, |fore| (fore.along + hind) * 0.5);
        // Лапы торчат из-под низа тела, как бы оно ни лежало
        let down = (build.dangle_down * s).max(self.tall(self.radii()) * s + LEG_PROTRUDE * s);
        let hang = frame.point(
            center,
            (along - middle + swing) * ratio[0] * s,
            side * build.dangle_side * ratio[1] * s,
            -down,
        );
        let hang = Vec3::new(hang.x, hang.y, hang.z.max(0.0));
        planted.lerp(hang, loose)
    }
}

/// Шея — дуга от основания к голове: сначала вверх, потом вперёд; `arch` — её изгиб.
/// Голова, опускающаяся вперёд, идёт дугой: шея не втягивается на полпути к земле.
/// Отведённая назад (чистка перьев) и поднятая голова не трогаются.
fn keep_length(reach: f32, up: f32, build: &Build) -> (f32, f32) {
    let [rest_reach, rest_up] = build.neck_rest;
    let length = reach.hypot(up);
    let least = rest_reach.hypot(rest_up) * NECK_KEEP;
    if reach <= 0.0 || up >= rest_up || length >= least || length <= 0.0 {
        return (reach, up);
    }
    let k = least / length;
    (reach * k, up * k)
}

fn neck(base: Vec3, head: Vec3, forward: Vec2, arch: f32) -> [Vec3; NECK_POINTS] {
    let reach = head - base;
    let span = reach.length();
    let raised = if span > 0.0 {
        (reach.z / span / NECK_RAISED).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let upright = base
        + Vec3::new(
            -forward.x * span * NECK_ARCH_BACK,
            -forward.y * span * NECK_ARCH_BACK,
            span * NECK_ARCH_UP,
        );
    // Без горба над спиной: шея выходит вперёд и плавно уходит вниз
    let droop = base
        + Vec3::new(
            reach.x * NECK_DROOP_REACH,
            reach.y * NECK_DROOP_REACH,
            span * NECK_DROOP_UP,
        );
    // Без изгиба шея — прямая от основания к голове
    let control = base.lerp(head, 0.5).lerp(droop.lerp(upright, raised), arch);
    std::array::from_fn(|i| {
        let t = i as f32 / (NECK_POINTS - 1) as f32;
        base.lerp(control, t).lerp(control.lerp(head, t), t)
    })
}
