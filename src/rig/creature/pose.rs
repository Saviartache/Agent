//! Готовая поза тела: всё, что нужно отрисовке, в пикселях мира с высотами над землёй.

use super::antic::{Antic, Look};
use crate::math::Vec2;
use crate::rig::RigKind;
use crate::rig::vec3::{Frame, Vec3};

/// Точек шеи от основания до головы.
pub const NECK_POINTS: usize = 10;
/// Глубина тела на экране сжата: без этого шея гуся, идущего от зрителя,
/// складывается с высотой и вытягивается вдвое.
pub const GROUND_SQUASH: f32 = 0.5;

/// Туловище — эллипсоид с полуосями вдоль хода, вбок и вверх.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub center: Vec3,
    /// Единичный ход по земле.
    pub forward: Vec2,
    /// Подъём носа, рад.
    pub pitch: f32,
    /// Поворот вокруг оси хода: π/2 — на боку, π — на спине.
    pub roll: f32,
    /// Наклон вбок вразвалку, рад: плюс — макушкой вправо (`Frame::rocked`).
    pub rock: f32,
    /// Полуоси: вперёд, вбок, вверх.
    pub radii: [f32; 3],
}

impl Body {
    /// Оси тела в мире: вперёд, вправо и «вверх по спине».
    pub fn frame(&self) -> Frame {
        let yaw = self.forward.y.atan2(self.forward.x);
        Frame::new(yaw, self.pitch, self.roll).rocked(yaw, self.rock)
    }

    /// Полуширина следа на земле: поперёк хода.
    pub fn footprint_width(&self) -> f32 {
        let (sin, cos) = self.roll.sin_cos();
        (self.radii[1] * cos).hypot(self.radii[2] * sin)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Head {
    pub center: Vec3,
    /// Единичный взгляд по земле.
    pub dir: Vec2,
    /// Подъём клюва, рад.
    pub pitch: f32,
    pub roll: f32,
    pub radius: f32,
    /// Клюв или рыло от поверхности головы, px.
    pub muzzle: f32,
    /// 0 — глаза закрыты, 1 — открыты.
    pub eyes_open: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pose {
    pub kind: RigKind,
    /// Точка на земле под телом; середина холста.
    pub pos: Vec2,
    /// Итоговый размер: масштаб ввода на «выскакивание».
    pub scale: f32,
    pub body: Body,
    pub neck: [Vec3; NECK_POINTS],
    pub head: Head,
    /// Лапы, левая и правая по ходу; `z > 0` — лапа в воздухе.
    /// У четвероногого это задняя пара.
    pub feet: [Vec3; 2],
    /// Передняя пара четвероногого.
    pub fore: Option<[Vec3; 2]>,
    /// Сила тени: 1 на земле, меньше в прыжке и в руках.
    pub shadow: f32,
    /// Насколько лапы поджаты под тело (сон).
    pub tuck: f32,
    /// Накладка сценки: отрисовка берёт из неё глаза, рот, крылья, узел.
    pub antic: Antic,
    pub look: Look,
}

/// Вектор тела на экране.
pub fn flatten(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.y * GROUND_SQUASH - v.z)
}

impl Pose {
    /// Точка тела на экране.
    pub fn screen(&self, point: Vec3) -> Vec2 {
        let ground = Vec3::on_ground(self.pos, 0.0);
        self.pos + flatten(point - ground)
    }

    /// Оси головы: вперёд — клюв или рыло.
    pub fn head_frame(&self) -> Frame {
        let head = &self.head;
        Frame::new(head.dir.y.atan2(head.dir.x), head.pitch, head.roll)
    }

    /// Кончик клюва или рыла.
    pub fn muzzle_tip(&self) -> Vec3 {
        let reach = self.head.radius * 0.7 + self.head.muzzle;
        self.head.center + self.head_frame().forward * reach
    }

    /// Насколько тело выше стойки: прыжок, полёт; px.
    pub fn rise(&self) -> f32 {
        (self.body.center.z - self.kind.build().stand_z * self.scale).max(0.0)
    }

    /// Сжимает позу вокруг точки на земле; `k` — доля от обычного размера.
    pub fn zoomed(&mut self, k: f32) {
        let about = Vec3::on_ground(self.pos, 0.0);
        let zoom = |p: Vec3| about + (p - about) * k;
        self.scale *= k;
        self.body.center = zoom(self.body.center);
        self.body.radii = self.body.radii.map(|r| r * k);
        self.neck = self.neck.map(zoom);
        self.head.center = zoom(self.head.center);
        self.head.radius *= k;
        self.head.muzzle *= k;
        self.feet = self.feet.map(zoom);
        self.fore = self.fore.map(|feet| feet.map(zoom));
    }
}
