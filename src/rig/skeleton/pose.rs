//! Поза скелета: поправки костей к покою и их положение в пространстве модели.

use super::Bone;
use crate::rig::vec3::{Frame, Vec3};

/// Камера смотрит на землю сверху под 30°.
const VIEW_SIN: f32 = 0.5;
const VIEW_COS: f32 = 0.866_025_4;
/// Высота на экране короче в `cos 30°` раз. «Земная» кость растягивает свои высоты
/// на столько, и на экране они идут один к одному, как у всего на рабочем столе.
const HEIGHT: f32 = 1.0 / VIEW_COS;

/// Точка модели глазами камеры: `x`, `y` — экран от точки на земле под телом,
/// `z` — глубина, меньше — ближе к зрителю. Шар остаётся кругом.
pub fn view(point: Vec3) -> Vec3 {
    Vec3::new(
        point.x,
        point.y * VIEW_SIN - point.z * VIEW_COS,
        -(point.y * VIEW_COS + point.z * VIEW_SIN),
    )
}

/// Поправка кости к позе покоя; `default()` ничего не меняет.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// Поворот от покоя: по земле, подъём носа, вокруг оси вперёд (`Frame::new`), рад.
    pub turn: [f32; 3],
    /// Сдвиг в осях родителя, design px.
    pub shift: Vec3,
    /// Растяжение по осям кости: её части и крепления детей; сами дети не тянутся.
    pub stretch: [f32; 3],
    /// Размер кости со всеми потомками.
    pub size: f32,
    /// Наклон вбок вразвалку, рад: плюс — макушкой вправо (`Frame::rocked`).
    pub rock: f32,
    /// Оси кости в мире как есть — вместо покоя и поворота: ухо, которое ставит решатель.
    pub frame: Option<Frame>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            turn: [0.0; 3],
            shift: Vec3::ZERO,
            stretch: [1.0; 3],
            size: 1.0,
            rock: 0.0,
            frame: None,
        }
    }
}

/// Кость в пространстве модели.
#[derive(Clone, Copy, Debug)]
pub struct Joint {
    pub origin: Vec3,
    pub frame: Frame,
    pub stretch: [f32; 3],
    pub size: f32,
    /// Высоты кости на экране один к одному (`HEIGHT`).
    pub ground: bool,
}

impl Joint {
    pub(super) fn root(size: f32) -> Self {
        Self {
            origin: Vec3::ZERO,
            frame: Frame::IDENTITY,
            stretch: [1.0; 3],
            size,
            ground: false,
        }
    }

    pub(super) fn child(&self, bone: &Bone, channel: &Channel) -> Self {
        let [yaw, pitch, roll] = channel.turn;
        Self {
            origin: self.point(bone.at + channel.shift),
            frame: channel.frame.unwrap_or_else(|| {
                let turned = Frame::new(yaw, pitch, roll).rocked(yaw, channel.rock);
                self.frame.compose(&bone.rest).compose(&turned)
            }),
            stretch: channel.stretch,
            size: self.size * channel.size,
            ground: bone.ground,
        }
    }

    /// Смещение в осях кости — в пространстве модели.
    pub fn vector(&self, local: Vec3) -> Vec3 {
        let [x, y, z] = self.stretch;
        let v = self
            .frame
            .rotate(Vec3::new(local.x * x, local.y * y, local.z * z))
            * self.size;
        if self.ground {
            Vec3::new(v.x, v.y, v.z * HEIGHT)
        } else {
            v
        }
    }

    /// Точка в осях кости — в пространстве модели.
    pub fn point(&self, local: Vec3) -> Vec3 {
        self.origin + self.vector(local)
    }
}

/// Все кости скелета в пространстве модели; заполняет `Skeleton::solve`.
#[derive(Clone, Debug, Default)]
pub struct World {
    pub(super) joints: Vec<Joint>,
}

impl World {
    /// В порядке костей скелета.
    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }
}
