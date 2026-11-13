//! Сложение вида: чем гусь, свинка и слоник отличаются при общем скелете.
//! Сложение вида: чем гусь, свинка и енот отличаются при общем скелете.

use super::feet::Stance;
use crate::math::Vec2;
use crate::rig::RigKind;

/// Размеры — design px (умножаются на масштаб), углы — радианы.
pub struct Build {
    /// Полуоси туловища: вперёд, вбок, вверх.
    pub body_radii: [f32; 3],
    /// Высота центра тела над землёй в стойке.
    pub stand_z: f32,
    pub body_pitch: f32,
    /// Основание шеи в осях тела: вперёд, вверх.
    pub neck_base: [f32; 2],
    /// Голова от основания шеи в покое: вперёд по земле, вверх.
    pub neck_rest: [f32; 2],
    /// Голова спящего в осях тела: вперёд, вбок, вверх.
    pub curl_rest: [f32; 3],
    /// Гибкость шеи: доля гусиного размаха жестов и отставания головы.
    pub neck_slack: f32,
    /// Может идти боком к ходу; иначе на ходу тело всегда развёрнуто по движению.
    pub sidesteps: bool,
    /// Голова поворачивается от оси тела не больше, рад.
    pub head_turn: f32,
    pub head_radius: f32,
    pub head_pitch: f32,
    /// Клюв или рыло от поверхности головы.
    pub muzzle: f32,
    /// Радиус отпечатка лапы.
    pub print: f32,
    /// Задняя (у птицы единственная) пара ног.
    pub hind: Pair,
    /// Передняя пара четвероногого.
    pub fore: Option<Pair>,
    /// Ноги в руках висят под телом: разнос от оси и расстояние вниз от центра.
    pub dangle_side: f32,
    pub dangle_down: f32,
    /// Хобот слона: вместо клюва и рыла.
    pub trunk: Option<Trunk>,
}

/// Хобот: гибкий, кончик на пружине.
pub struct Trunk {
    pub length: f32,
    /// Наклон к земле в покое, рад: висит вниз.
    pub droop: f32,
    /// Во сколько раз наклон головы поднимает или опускает хобот.
    pub gain: f32,
    /// Доля гусиного отставания кончика.
    pub slack: f32,
    /// Крадётся: на ходу тело ниже стойки на столько.
    pub prowl: f32,
}

const GOOSE: Build = Build {
    body_radii: [15.0, 10.0, 9.5],
    stand_z: 11.5,
    body_pitch: 0.2,
    neck_base: [10.0, 5.0],
    neck_rest: [6.0, 13.5],
    curl_rest: [-2.0, 4.5, 8.5],
    neck_slack: 1.0,
    sidesteps: true,
    head_turn: 1.4,
    head_radius: 5.0,
    head_pitch: -0.12,
    muzzle: 6.5,
    print: 2.9,
    // Лапы у гуся ближе к хвосту, чем к груди
    hind: Pair {
        along: -4.0,
        side: 3.5,
    },
    fore: None,
    dangle_side: 4.5,
    dangle_down: 12.0,
    trunk: None,
    prowl: 0.0,
};

/// Копилка на коротких ногах: голова почти с тело и сидит на нём без шеи.
const PIG: Build = Build {
    body_radii: [12.5, 10.0, 9.5],
    stand_z: 12.0,
    body_pitch: 0.0,
    neck_base: [8.0, 2.0],
    neck_rest: [5.0, 2.5],
    curl_rest: [12.0, 1.5, 2.0],
    neck_slack: 0.3,
    sidesteps: false,
    head_turn: 0.8,
    head_radius: 8.0,
    head_pitch: 0.0,
    muzzle: 5.0,
    print: 1.8,
    // Задние ноги стоят у́же передних: четыре следа — четыре дорожки
    hind: Pair {
        along: -7.0,
        side: 4.2,
    },
    fore: Some(Pair {
        along: 7.5,
        side: 5.4,
    }),
    dangle_side: 5.0,
    dangle_down: 11.0,
    trunk: None,
};

/// Пухлый слонёнок на ногах-тумбах: голова на теле без шеи, хобот висит.
const ELEPHANT: Build = Build {
    body_radii: [14.0, 11.0, 10.5],
    stand_z: 14.0,
    body_pitch: 0.0,
    neck_base: [9.5, 3.5],
    neck_rest: [4.5, 3.0],
    curl_rest: [13.0, 1.0, 1.5],
    neck_slack: 0.3,
    sidesteps: false,
    head_turn: 0.8,
    head_radius: 10.0,
    head_pitch: 0.0,
    muzzle: 0.0,
    print: 2.6,
    hind: Pair {
        along: -7.5,
        side: 5.0,
    },
    fore: Some(Pair {
        along: 7.5,
        side: 5.6,
    }),
    dangle_side: 5.5,
    dangle_down: 12.0,
    trunk: Some(Trunk {
        length: 13.0,
        droop: -1.2,
        gain: 2.2,
        slack: 0.6,
    }),
};

/// Такса: длинное низкое тело на очень коротких ногах, заметная шея, вытянутая морда.
const DACHSHUND: Build = Build {
    body_radii: [18.0, 6.2, 6.2],
    stand_z: 7.8,
    body_pitch: 0.0,
    neck_base: [13.0, 3.0],
    neck_rest: [5.0, 8.0],
    curl_rest: [21.0, 2.0, 0.0],
    neck_slack: 0.6,
    sidesteps: false,
    head_turn: 1.1,
    head_radius: 5.2,
    head_pitch: -0.1,
    muzzle: 6.0,
    print: 1.6,
    hind: Pair {
        along: -12.0,
        side: 3.4,
    },
    fore: Some(Pair {
        along: 11.5,
        side: 3.8,
    }),
    dangle_side: 3.8,
    dangle_down: 8.5,
    prowl: 0.0,
};

/// Длинное низкое тело с горбатой спиной, острая морда на короткой шее; крадётся.
const RACCOON: Build = Build {
    body_radii: [12.0, 8.0, 7.0],
    stand_z: 10.4,
    // Зад выше плеч
    body_pitch: -0.12,
    neck_base: [9.5, 2.5],
    neck_rest: [5.5, 4.5],
    curl_rest: [10.0, 3.0, 2.5],
    neck_slack: 0.5,
    sidesteps: false,
    head_turn: 1.1,
    head_radius: 6.8,
    head_pitch: -0.05,
    muzzle: 5.5,
    print: 1.7,
    hind: Pair {
        along: -8.0,
        side: 3.8,
    },
    fore: Some(Pair {
        along: 8.0,
        side: 4.6,
    }),
    dangle_side: 4.5,
    dangle_down: 10.0,
    prowl: 3.5,
};

impl RigKind {
    /// Радиус отпечатка лапы, design px.
    pub fn print_radius(self) -> f32 {
        self.build().print
    }

    pub(super) fn build(self) -> &'static Build {
        match self {
            Self::Goose => &GOOSE,
            Self::Pig => &PIG,
            Self::Dachshund => &DACHSHUND,
            Self::Elephant => &ELEPHANT,
            Self::Raccoon => &RACCOON,
        }
    }
}

/// Пара ног: где она под телом.
pub struct Pair {
    /// От точки под телом по ходу.
    pub along: f32,
    /// Разнос ног от оси тела.
    pub side: f32,
}

impl Pair {
    /// Стойка пары у тела, стоящего в `pos`.
    pub fn stance(&self, pos: Vec2, yaw: f32, vel: Vec2, scale: f32, grounded: bool) -> Stance {
        let at = pos + Vec2::from_angle(yaw) * (self.along * scale);
        Stance::new(at, yaw, vel, scale, self.side * scale, grounded)
    }
}
