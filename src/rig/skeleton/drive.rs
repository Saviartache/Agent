//! Поза тела (`Pose`) водит скелет: кости и регуляторы ищутся по именам, каких нет —
//! те остаются в покое. Имена — общий словарь всех видов; чем вид отвечает на регулятор
//! (куда отводит крыло, как раскрывает клюв), решают драйверы костей в его `rig.json`.

use std::f32::consts::TAU;

use super::{Channel, Skeleton, World};
use crate::math::Vec2;
use crate::rig::creature::{Ear, Eyes, NECK_POINTS, Pose};
use crate::rig::vec3::{Frame, Vec3};

/// Ступни — задняя пара, потом передняя; слева, потом справа. Уши — левое, правое.
const FEET: [&str; 4] = ["foot_hind_l", "foot_hind_r", "foot_fore_l", "foot_fore_r"];
const EARS: [&str; 2] = ["ear_l", "ear_r"];
/// Регуляторы в порядке `Drive::values`.
const CONTROLS: [&str; 16] = [
    "lids",
    "eyes_crossed",
    "eyes_wide",
    "mouth",
    "mouth_open",
    "tongue",
    "beak_open",
    "gape",
    "mud",
    "snout_mud",
    "has_wings",
    "wings",
    "flap",
    "has_knot",
    "knot",
    "standing",
];
/// Раскрытый клюв: угол половинки при `mouth = 1`, рад; приоткрытый — виден язык.
const GAPE: f32 = 0.5;
const TONGUE_GAPE: f32 = 0.2;
/// Точка шеи, по которой находится изгиб её дуги.
const NECK_PROBE: usize = NECK_POINTS / 2;
/// Тело стоит: ногти и носки видны.
const UPRIGHT: f32 = 0.7;
/// Как далеко вперёд хобот выходит из морды, прежде чем свернуть к кончику, доля длины.
const TRUNK_BEND: f32 = 0.45;

#[derive(Clone, Debug)]
pub struct Drive {
    body: Option<usize>,
    head: Option<usize>,
    feet: [Option<usize>; 4],
    ears: [Option<usize>; 2],
    /// Опорная точка дуги шеи и её середина (узел).
    neck_bend: Option<usize>,
    neck_mid: Option<usize>,
    tail_tip: Option<usize>,
    trunk_bend: Option<usize>,
    trunk_tip: Option<usize>,
    slots: [Option<usize>; 16],
    channels: Vec<Channel>,
    controls: Vec<f32>,
}

fn flag(on: bool) -> f32 {
    f32::from(u8::from(on))
}

impl Drive {
    pub fn new(skeleton: &Skeleton) -> Self {
        Self {
            body: skeleton.bone("body"),
            head: skeleton.bone("head"),
            feet: FEET.map(|name| skeleton.bone(name)),
            ears: EARS.map(|name| skeleton.bone(name)),
            neck_bend: skeleton.bone("neck_bend"),
            neck_mid: skeleton.bone("neck_mid"),
            tail_tip: skeleton.bone("tail_tip"),
            trunk_bend: skeleton.bone("trunk_bend"),
            trunk_tip: skeleton.bone("trunk_tip"),
            slots: CONTROLS.map(|name| skeleton.control(name)),
            channels: vec![Channel::default(); skeleton.bones().len()],
            controls: vec![0.0; skeleton.controls().len()],
        }
    }

    /// Регуляторы после `apply`, в порядке скелета.
    pub fn controls(&self) -> &[f32] {
        &self.controls
    }

    /// Значения регуляторов в порядке `CONTROLS`.
    fn values(pose: &Pose) -> [f32; 16] {
        let (antic, look) = (&pose.antic, &pose.look);
        let open = antic.mouth.clamp(0.0, 1.0);
        let gape = (open * GAPE).max(if antic.tongue { TONGUE_GAPE } else { 0.0 });
        let wings = antic.wings.clamp(0.0, 1.0);
        let knot = antic.knot.clamp(0.0, 1.0);
        let standing = pose.body.frame().up.z > UPRIGHT && pose.tuck <= 0.0;
        [
            flag(pose.head.eyes_open < 0.5),
            flag(antic.eyes == Eyes::Crossed),
            flag(antic.eyes == Eyes::Wide),
            flag(open > 0.0),
            open,
            flag(antic.tongue),
            flag(gape > 0.0),
            gape,
            look.mud,
            look.snout,
            flag(wings > 0.0),
            wings,
            (antic.flap * TAU).sin() * wings,
            flag(knot > 0.0),
            knot,
            flag(standing),
        ]
    }

    /// Ставит кости скелета по позе. Тело, голова, ступни, уши и опорные точки шеи и хобота
    /// должны висеть на корне: поза задаёт их от точки на земле.
    pub fn apply(&mut self, skeleton: &Skeleton, pose: &Pose, world: &mut World) {
        if pose.scale.is_nan() || pose.scale <= 0.0 {
            world.joints.clear();
            return;
        }
        let ground = Vec3::on_ground(pose.pos, 0.0);
        // Сдвиг от места кости в покое: файл вида остаётся позой покоя
        let rest = |bone: usize| skeleton.bones()[bone].at;
        let place = |bone: usize, at: Vec3| (at - ground) * (1.0 / pose.scale) - rest(bone);
        let yaw = |dir: Vec2| dir.y.atan2(dir.x);
        let (body, head) = (&pose.body, &pose.head);
        let body_yaw = yaw(body.forward);
        let mut set = |bone: Option<usize>, edit: &mut dyn FnMut(usize, &mut Channel)| {
            if let Some(bone) = bone {
                edit(bone, &mut self.channels[bone]);
            }
        };
        set(self.body, &mut |bone, channel| {
            channel.shift = place(bone, body.center);
            channel.turn = [body_yaw, body.pitch, body.roll];
            channel.rock = body.rock;
            channel.stretch = pose.body_stretch();
        });
        set(self.head, &mut |bone, channel| {
            channel.shift = place(bone, head.center);
            channel.turn = [yaw(head.dir), head.pitch, head.roll];
        });
        let feet = pose.feet.into_iter().chain(pose.fore.into_iter().flatten());
        for (bone, foot) in self.feet.into_iter().zip(feet) {
            set(bone, &mut |bone, channel| {
                channel.shift = place(bone, foot);
                // Носок смотрит по ходу тела; поджатая лапа уходит в тело
                channel.turn[0] = body_yaw;
                channel.size = 1.0 - pose.tuck;
            });
        }
        // Дуга шеи: её опорная точка восстанавливается по пробе на кривой
        let (base, end, probe) = (pose.neck[0], head.center, pose.neck[NECK_PROBE]);
        let t = NECK_PROBE as f32 / (NECK_POINTS - 1) as f32;
        let bend = (probe - base * ((1.0 - t) * (1.0 - t)) - end * (t * t))
            * (1.0 / (2.0 * t * (1.0 - t)));
        set(self.neck_bend, &mut |bone, channel| {
            channel.shift = place(bone, bend)
        });
        set(self.neck_mid, &mut |bone, channel| {
            channel.shift = place(bone, probe)
        });
        // Кончик хвоста отстаёт по земле: сдвиг мира — в оси тела
        let lag = Vec3::on_ground(pose.tail, 0.0) * (1.0 / pose.scale);
        let (frame, stretch) = (body.frame(), pose.body_stretch());
        set(self.tail_tip, &mut |_, channel| {
            channel.shift = Vec3::new(
                lag.dot(frame.forward) / stretch[0].max(f32::EPSILON),
                lag.dot(frame.right) / stretch[1].max(f32::EPSILON),
                lag.dot(frame.up) / stretch[2].max(f32::EPSILON),
            );
        });
        if let Some([root, tip]) = pose.trunk {
            // Хобот выходит из морды вперёд и гнётся к кончику
            let bend = root + pose.head_frame().forward * ((tip - root).length() * TRUNK_BEND);
            set(self.trunk_bend, &mut |bone, channel| {
                channel.shift = place(bone, bend)
            });
            set(self.trunk_tip, &mut |bone, channel| {
                channel.shift = place(bone, tip)
            });
        }
        for (bone, ear) in self.ears.into_iter().zip(pose.ears.into_iter().flatten()) {
            set(bone, &mut |bone, channel| {
                channel.shift = place(bone, ear.root);
                (channel.frame, channel.stretch) = ear_frame(&ear, pose.scale);
            });
        }
        for (slot, value) in self.slots.into_iter().zip(Self::values(pose)) {
            if let Some(slot) = slot {
                self.controls[slot] = value;
            }
        }
        skeleton.solve(&self.channels, &self.controls, pose.scale, world);
    }
}

/// Оси уха: вперёд — лопасть наружу от шарнира, вправо — вниз; растяжение — их длины,
/// а по толщине — знак: «вверх» кости смотрит туда же, куда изнанка.
fn ear_frame(ear: &Ear, scale: f32) -> (Option<Frame>, [f32; 3]) {
    let [blade, hang] = ear.axes;
    let (forward, right) = (blade.normalized(), hang.normalized());
    let up = forward.cross(right).normalized();
    if up == Vec3::ZERO {
        return (None, [0.0; 3]);
    }
    let inner = if ear.inner.dot(up) < 0.0 { -1.0 } else { 1.0 };
    let frame = Frame { forward, right, up };
    (
        Some(frame),
        [blade.length() / scale, hang.length() / scale, inner],
    )
}
