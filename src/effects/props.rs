//! Предметы сценок на земле: мяч, яблоко, жёлудь, кожура, кучка, яйцо, монетки, ведро, бумажки.
//! Простая физика: высота с тяжестью и отскоком, трение о землю, отскок от краёв.
//! Предметы не карточки: мышь их не ловит, приложению о них не сообщают.

use super::{bounding, sane_dt};
use crate::math::{Rect, Vec2};

/// Логические px/с².
const GRAVITY: f32 = 900.0;
/// Доля вертикальной скорости, с которой предмет отскакивает от земли.
const GROUND_BOUNCE: f32 = 0.45;
/// Медленнее — уже лежит, px/с.
const SETTLE_SPEED: f32 = 40.0;
const WALL_BOUNCE: f32 = 0.7;
/// Гаснет за столько секунд до конца жизни.
const FADE_SECS: f32 = 0.5;
/// Плавно падающее: перо, бабочка, падающая скорость не больше этой, px/с.
const DRIFT_FALL: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Ball,
    Apple,
    Acorn,
    /// Банановая кожура.
    Peel,
    /// Кучка свинки; рисуется с мухами.
    Pile,
    Egg,
    Coin,
    Bucket,
    Crumb,
    Pebble,
    Feather,
    /// Нарисованная стрелочка курсора: настоящий курсор не трогаем.
    Arrow,
    Butterfly,
    /// Скомканная бумажка или записка енота.
    Paper,
}

impl PropKind {
    /// Что рисунок занимает над опорой сверх обычного: высота центра и радиус круга,
    /// в радиусах. Мяч стоит на земле, мухи вьются над кучкой.
    fn crown(self) -> (f32, f32) {
        match self {
            Self::Ball => (0.9, 1.5),
            Self::Pile => (2.3, 2.6),
            _ => (0.7, 1.5),
        }
    }

    /// Радиус, логические px.
    pub fn radius(self) -> f32 {
        match self {
            Self::Ball => 9.0,
            Self::Apple => 7.0,
            Self::Acorn => 5.0,
            Self::Peel => 7.0,
            Self::Pile => 8.0,
            Self::Egg => 6.0,
            Self::Coin => 4.0,
            Self::Bucket => 12.0,
            Self::Crumb => 2.0,
            Self::Pebble => 4.0,
            Self::Feather => 5.0,
            Self::Arrow => 8.0,
            Self::Butterfly => 6.0,
            Self::Paper => 4.5,
        }
    }

    /// Трение о землю, 1/с.
    fn friction(self) -> f32 {
        match self {
            Self::Ball => 0.8,
            Self::Peel => 1.5,
            _ => 4.0,
        }
    }

    /// Не падает камнем: парит вниз (перо, бумажка) или держится в воздухе (бабочка).
    fn drifts(self) -> bool {
        matches!(self, Self::Feather | Self::Butterfly | Self::Paper)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropId(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub kind: PropKind,
    /// Точка на земле под предметом, физические px.
    pub pos: Vec2,
    /// Высота над землёй, px.
    pub z: f32,
    pub vel: Vec2,
    /// Вверх — плюс.
    pub vz: f32,
    pub angle: f32,
    /// Вращение, рад/с.
    pub spin: f32,
    /// Своё у вида: откушено, трещин, монеток.
    pub stage: u8,
    pub radius: f32,
    pub age: f32,
    life: f32,
    id: PropId,
}

impl Prop {
    pub fn id(&self) -> PropId {
        self.id
    }

    pub fn opacity(&self) -> f32 {
        ((self.life - self.age) / FADE_SECS).clamp(0.0, 1.0)
    }

    /// Лежит на земле и не катится.
    pub fn resting(&self) -> bool {
        self.z <= 0.0 && self.vz == 0.0 && self.vel.length() < 1.0
    }

    /// Дожить ещё не больше `secs`: исчезнет, плавно погаснув.
    pub fn expire_in(&mut self, secs: f32) {
        self.life = self.life.min(self.age + secs.max(0.0));
    }
}

#[derive(Clone, Debug)]
pub struct Props {
    live: Vec<Prop>,
    capacity: usize,
    next: u32,
}

impl Props {
    pub fn new(capacity: usize) -> Self {
        Self {
            live: Vec::with_capacity(capacity),
            capacity: capacity.max(1),
            next: 0,
        }
    }

    /// Предмет вида `kind` в `pos` на высоте `z`; `ui` — масштаб; живёт `ttl` секунд.
    pub fn spawn(&mut self, kind: PropKind, pos: Vec2, z: f32, ui: f32, ttl: f32) -> PropId {
        if self.live.len() == self.capacity {
            self.live.remove(0);
        }
        let id = PropId(self.next);
        self.next = self.next.wrapping_add(1);
        let clean = |v: f32| if v.is_finite() { v } else { 0.0 };
        self.live.push(Prop {
            kind,
            pos: Vec2::new(clean(pos.x), clean(pos.y)),
            z: clean(z).max(0.0),
            vel: Vec2::ZERO,
            vz: 0.0,
            angle: 0.0,
            spin: 0.0,
            stage: 0,
            radius: kind.radius() * ui.max(0.0),
            age: 0.0,
            life: clean(ttl).max(FADE_SECS),
            id,
        });
        id
    }

    pub fn get(&self, id: PropId) -> Option<&Prop> {
        self.live.iter().find(|prop| prop.id == id)
    }

    pub fn get_mut(&mut self, id: PropId) -> Option<&mut Prop> {
        self.live.iter_mut().find(|prop| prop.id == id)
    }

    pub fn remove(&mut self, id: PropId) {
        self.live.retain(|prop| prop.id != id);
    }

    /// `area` — где предметы отскакивают от краёв; `ui` — масштаб тяжести.
    pub fn update(&mut self, dt: f32, area: Rect, ui: f32) {
        let dt = sane_dt(dt);
        let gravity = GRAVITY * ui;
        self.live.retain_mut(|prop| {
            let step = dt.min(prop.life - prop.age);
            prop.age += step;
            fly(prop, step, gravity, ui);
            bounce_off(prop, area);
            prop.age < prop.life
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = &Prop> + '_ {
        self.live.iter()
    }

    /// С запасом на высоту полёта.
    pub fn bounds(&self) -> Option<Rect> {
        bounding(self.live.iter().flat_map(|prop| {
            let lifted = prop.pos - Vec2::new(0.0, prop.z);
            let (rise, reach) = prop.kind.crown();
            [
                (prop.pos, prop.radius * 1.5),
                (lifted, prop.radius * 1.5),
                (
                    lifted - Vec2::new(0.0, prop.radius * rise),
                    prop.radius * reach,
                ),
            ]
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

fn fly(prop: &mut Prop, dt: f32, gravity: f32, ui: f32) {
    let finite = |v: Vec2| v.x.is_finite() && v.y.is_finite();
    if !finite(prop.vel) || !prop.vz.is_finite() || !prop.spin.is_finite() {
        (prop.vel, prop.vz, prop.spin) = (Vec2::ZERO, 0.0, 0.0);
    }
    if prop.kind.drifts() {
        if prop.kind != PropKind::Butterfly && prop.z > 0.0 {
            prop.vz = (prop.vz - gravity * dt).max(-DRIFT_FALL * ui);
        }
    } else if prop.z > 0.0 || prop.vz > 0.0 {
        prop.vz -= gravity * dt;
    }
    prop.z += prop.vz * dt;
    if prop.z <= 0.0 {
        prop.z = 0.0;
        prop.vz = if -prop.vz > SETTLE_SPEED * ui && !prop.kind.drifts() {
            -prop.vz * GROUND_BOUNCE
        } else {
            0.0
        };
        prop.vel = prop.vel * (-prop.kind.friction() * dt).exp();
        prop.spin *= (-prop.kind.friction() * dt).exp();
        if prop.kind == PropKind::Ball {
            // Катится без проскальзывания
            let way = if prop.vel.x < 0.0 { -1.0 } else { 1.0 };
            prop.spin = way * prop.vel.length() / prop.radius;
        }
    }
    prop.pos += prop.vel * dt;
    prop.angle += prop.spin * dt;
}

fn bounce_off(prop: &mut Prop, area: Rect) {
    let r = prop.radius;
    if prop.pos.x < area.min.x + r || prop.pos.x > area.max.x - r {
        prop.vel.x = -prop.vel.x * WALL_BOUNCE;
    }
    if prop.pos.y < area.min.y + r || prop.pos.y > area.max.y - r {
        prop.vel.y = -prop.vel.y * WALL_BOUNCE;
    }
    prop.pos = area.clamp(prop.pos, r);
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const AREA: Rect = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));

    fn run(props: &mut Props, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            props.update(DT, AREA, 1.0);
        }
    }

    #[test]
    fn dropped_prop_bounces_and_settles() {
        let mut props = Props::new(4);
        let id = props.spawn(PropKind::Apple, Vec2::new(100.0, 100.0), 80.0, 1.0, 30.0);
        let mut bounced = false;
        let mut last = 80.0;
        for _ in 0..120 {
            props.update(DT, AREA, 1.0);
            let z = props.get(id).unwrap().z;
            bounced |= z > last && last == 0.0;
            last = z;
        }
        assert!(bounced);
        run(&mut props, 2.0);
        assert!(props.get(id).unwrap().resting());
    }

    #[test]
    fn kicked_ball_bounces_off_the_edges_and_rolls_to_a_stop() {
        let mut props = Props::new(4);
        let id = props.spawn(PropKind::Ball, Vec2::new(700.0, 300.0), 0.0, 1.0, 60.0);
        props.get_mut(id).unwrap().vel = Vec2::new(600.0, 0.0);
        run(&mut props, 0.5);
        let ball = props.get(id).unwrap();
        assert!(ball.vel.x < 0.0 && ball.pos.x <= 800.0 - ball.radius);
        run(&mut props, 12.0);
        assert!(props.get(id).unwrap().resting());
    }

    #[test]
    fn rolling_ball_turns_with_its_travel() {
        let mut props = Props::new(4);
        let id = props.spawn(PropKind::Ball, Vec2::new(100.0, 300.0), 0.0, 1.0, 60.0);
        props.get_mut(id).unwrap().vel = Vec2::new(-200.0, 0.0);
        run(&mut props, 0.2);
        let ball = props.get(id).unwrap();
        let rolled = 100.0 - ball.pos.x;
        assert!(ball.angle < 0.0, "{}", ball.angle);
        assert!((-ball.angle * ball.radius - rolled).abs() < rolled * 0.1);
    }

    #[test]
    fn feather_drifts_down_slowly_and_props_expire() {
        let mut props = Props::new(4);
        let id = props.spawn(PropKind::Feather, Vec2::new(100.0, 100.0), 60.0, 1.0, 5.0);
        run(&mut props, 1.0);
        let z = props.get(id).unwrap().z;
        assert!(z > 60.0 - DRIFT_FALL * 1.2 && z < 60.0, "{z}");
        run(&mut props, 5.0);
        assert!(props.is_empty());
    }

    #[test]
    fn capacity_and_broken_input_hold() {
        let mut props = Props::new(2);
        let first = props.spawn(PropKind::Coin, Vec2::ZERO, 0.0, 1.0, 9.0);
        props.spawn(
            PropKind::Coin,
            Vec2::new(f32::NAN, 0.0),
            f32::NAN,
            1.0,
            f32::NAN,
        );
        props.spawn(PropKind::Coin, Vec2::ZERO, 0.0, 1.0, 9.0);
        assert_eq!(props.iter().count(), 2);
        assert!(props.get(first).is_none());
        props.update(f32::NAN, AREA, 1.0);
        props.update(1e9, AREA, 1.0);
        assert!(props.is_empty());
    }
}
