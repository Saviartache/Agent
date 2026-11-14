//! Скелет: дерево костей с частями и сокетами. Вид тела — данные (`rig.json`), а не код;
//! порядок слоёв не задаётся — его решает глубина при отрисовке.

mod drive;
mod file;
mod pose;
#[cfg(test)]
mod tests;

pub use self::drive::Drive;
pub use self::file::RIG_FILE;
pub use self::pose::{Channel, Joint, World, view};
use super::vec3::{Frame, Vec3};

/// Кость: место и поворот в осях родителя в позе покоя. Размеры — design px.
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    /// Всегда раньше самой кости.
    pub parent: Option<usize>,
    pub at: Vec3,
    pub rest: Frame,
    /// Растяжение в покое: ноль прячет кость, пока её не раскроет регулятор.
    pub stretch: [f32; 3],
    /// Высоты кости (её части и крепления детей) на экране один к одному.
    pub ground: bool,
    pub drivers: Vec<Driver>,
}

/// Регулятор двигает кость: его значение умножается на эти добавки к покою.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Driver {
    pub control: usize,
    /// Рад.
    pub turn: [f32; 3],
    pub shift: Vec3,
    pub stretch: [f32; 3],
}

/// Форма части в осях кости: `x` — вперёд, `y` — вправо, `z` — вверх.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Ball {
        at: Vec3,
        radius: f32,
    },
    /// Полуоси вдоль осей кости.
    Ellipsoid {
        at: Vec3,
        radii: [f32; 3],
    },
    /// Отрезок с круглыми концами; радиусы на концах разные.
    Capsule {
        from: Vec3,
        to: Vec3,
        radii: [f32; 2],
    },
    /// Плоский эллипс с полуосями `axes`; виден только со стороны `axes[0] × axes[1]`.
    Disc {
        at: Vec3,
        axes: [Vec3; 2],
    },
    /// Гибкая связка костей: от точки `from` этой кости через начала костей `through`.
    Chain {
        from: Vec3,
        through: Vec<usize>,
        /// Радиусы вдоль цепочки, от первого к последнему, через равные доли.
        radii: Vec<f32>,
        /// Длиннее этого цепочка не тянется: дальний конец отрывается от цели.
        reach: Option<f32>,
        /// Плавная дуга из стольких звеньев: кости — опорные точки кривой, а не её узлы.
        curve: Option<usize>,
        /// Цвета звеньев по кругу, считая от конца: кольца хвоста. Пусто — цвет части.
        bands: Vec<u32>,
        /// Под землю цепочка не уходит.
        floor: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub bone: usize,
    /// `0xRRGGBB`.
    pub color: u32,
    pub shape: Shape,
    /// Кайма: номер обводки. Части одной обводки сливаются в силуэт без швов.
    pub rim: Option<usize>,
    /// Часть лицом к зрителю: точка привязки на кости, а координаты формы отложены от неё
    /// в осях экрана — вправо, вверх, к зрителю. Зрачок, веко, рот.
    pub facing: Option<Vec3>,
    /// Доля, на которую привязка уходит к началу кости, когда та отвернулась от зрителя.
    pub sink: f32,
    /// Сидя на дальней от зрителя стороне кости или на отвернувшейся кости, часть целиком
    /// уходит за неё: глаз на затылке выглядывает из-за головы, а не лежит на ней.
    pub behind: bool,
    /// На столько часть ближе к зрителю, чем стоит: рисунок на поверхности (живот, ноготь)
    /// виден поверх неё, а с обратной стороны по-прежнему скрыт. Design px.
    pub lift: f32,
    /// Регуляторы: часть видна на произведение `show` и не видна на `hide`.
    pub show: Vec<usize>,
    pub hide: Vec<usize>,
}

/// Именованная точка на кости: сюда крепят предметы и несут карточки.
#[derive(Clone, Debug)]
pub struct Socket {
    pub name: String,
    pub bone: usize,
    pub at: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    /// `0xRRGGBB`.
    pub color: u32,
    /// design px.
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    bones: Vec<Bone>,
    parts: Vec<Part>,
    sockets: Vec<Socket>,
    controls: Vec<String>,
    outlines: Vec<Outline>,
}

impl Skeleton {
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Обводки; на них ссылается `Part::rim`.
    pub fn outlines(&self) -> &[Outline] {
        &self.outlines
    }

    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }

    pub fn socket(&self, name: &str) -> Option<usize> {
        self.sockets.iter().position(|socket| socket.name == name)
    }

    /// Регуляторы вида: веки, грязь, рот, крылья.
    pub fn controls(&self) -> &[String] {
        &self.controls
    }

    pub fn control(&self, name: &str) -> Option<usize> {
        self.controls.iter().position(|control| control == name)
    }

    /// Сокет в пространстве модели; `None` — нет такого или поза не от этого скелета.
    pub fn socket_point(&self, world: &World, socket: usize) -> Option<Vec3> {
        let socket = self.sockets.get(socket)?;
        Some(world.joints().get(socket.bone)?.point(socket.at))
    }

    /// Кости в пространстве модели: начало — точка на земле под телом, единица — px.
    /// `channels` — по одной поправке на кость, недостающие — покой; `controls` —
    /// регуляторы по порядку, недостающие — нули; `size` — масштаб.
    pub fn solve(&self, channels: &[Channel], controls: &[f32], size: f32, world: &mut World) {
        world.joints.clear();
        for (index, bone) in self.bones.iter().enumerate() {
            let mut channel = channels.get(index).copied().unwrap_or_default();
            let mut stretch = bone.stretch;
            for driver in &bone.drivers {
                let value = controls.get(driver.control).copied().unwrap_or(0.0);
                channel.shift = channel.shift + driver.shift * value;
                for axis in 0..3 {
                    channel.turn[axis] += driver.turn[axis] * value;
                    stretch[axis] += driver.stretch[axis] * value;
                }
            }
            for axis in 0..3 {
                channel.stretch[axis] *= stretch[axis];
            }
            let parent = match bone.parent {
                Some(parent) => world.joints[parent],
                None => Joint::root(size),
            };
            world.joints.push(parent.child(bone, &channel));
        }
    }
}
