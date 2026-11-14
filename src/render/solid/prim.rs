//! Части тела глазами камеры (`rig::view`): `x`, `y` — экран, `z` — глубина. Луч зрителя
//! идёт вдоль `z`, поэтому шар — круг, капсула — плоская капсула, а глубина точки
//! считается в несколько действий. Всё, что не меняется вдоль строки, считается один раз.

use crate::rig::{Joint, Part, Shape, Vec3, view};

const EPSILON: f32 = 1e-6;
/// Кость отвернулась от зрителя настолько, что лица не видно (глубина её «вперёд»).
const TURNED_AWAY: f32 = 0.25;
/// Часть на дальней стороне кости, когда её вылет смотрит от зрителя на такую долю:
/// в точный профиль она ещё видна.
const FAR_SIDE: f32 = 0.2;
/// На столько своих вылетов часть уходит вглубь, прячась за кость.
const BEHIND: f32 = 3.0;

/// Часть, которую можно пройти строками экрана.
pub(super) trait Scan {
    /// То, что в строке не меняется.
    type Row;
    /// Охват на экране: лево, верх, право, низ.
    fn bounds(&self) -> [f32; 4];
    /// Отрезок строки `y`, вне которого части нет.
    fn row(&self, y: f32) -> Option<(f32, f32, Self::Row)>;
    /// Глубина ближней поверхности в `x`. `limit` — глубина уже нарисованного: то, что
    /// заведомо за ним, можно не считать.
    fn depth(&self, row: &Self::Row, x: f32, limit: f32) -> Option<f32>;
}

pub(super) struct Ball {
    center: Vec3,
    radius: f32,
}

impl Scan for Ball {
    /// Квадрат полухорды строки.
    type Row = f32;

    fn bounds(&self) -> [f32; 4] {
        let (c, r) = (self.center, self.radius);
        [c.x - r, c.y - r, c.x + r, c.y + r]
    }

    fn row(&self, y: f32) -> Option<(f32, f32, f32)> {
        let dy = y - self.center.y;
        let chord = self.radius * self.radius - dy * dy;
        (chord >= 0.0).then(|| {
            let half = chord.sqrt();
            (self.center.x - half, self.center.x + half, chord)
        })
    }

    fn depth(&self, chord: &f32, x: f32, limit: f32) -> Option<f32> {
        let dx = x - self.center.x;
        let h = chord - dx * dx;
        if h < 0.0 {
            return None;
        }
        // Вся толща шара здесь за нарисованным: корень не нужен
        let behind = self.center.z - limit;
        if behind > 0.0 && h <= behind * behind {
            return None;
        }
        Some(self.center.z - h.sqrt())
    }
}

/// Эллипсоид с любыми полуосями: в своих осях он единичный шар.
pub(super) struct Quadric {
    center: Vec3,
    axes: [Vec3; 3],
    reach: [f32; 2],
    /// Силуэт: `p·dx² + 2q·dx·dy + s·dy² ≤ a`.
    p: f32,
    q: f32,
    s: f32,
    a: f32,
    /// Глубина: `z = center.z − (bx·dx + by·dy + √h) / a`.
    bx: f32,
    by: f32,
}

impl Quadric {
    fn new(center: Vec3, axes: [Vec3; 3]) -> Option<Self> {
        let det = axes[0].dot(axes[1].cross(axes[2]));
        // Сплющенный в лист: объёма нет
        let volume = axes[0].length() * axes[1].length() * axes[2].length();
        if det.abs() <= 1e-4 * volume || !det.is_finite() {
            return None;
        }
        // Строки обратной матрицы: точка в осях эллипсоида
        let rows = [
            axes[1].cross(axes[2]) * (1.0 / det),
            axes[2].cross(axes[0]) * (1.0 / det),
            axes[0].cross(axes[1]) * (1.0 / det),
        ];
        let column = |pick: fn(Vec3) -> f32| Vec3::new(pick(rows[0]), pick(rows[1]), pick(rows[2]));
        let (x, y, d) = (column(|v| v.x), column(|v| v.y), column(|v| v.z));
        let (a, bx, by) = (d.dot(d), x.dot(d), y.dot(d));
        let extent = |pick: fn(Vec3) -> f32| {
            axes.iter()
                .map(|axis| pick(*axis).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        Some(Self {
            center,
            axes,
            reach: [extent(|v| v.x), extent(|v| v.y)],
            p: a * x.dot(x) - bx * bx,
            q: a * x.dot(y) - bx * by,
            s: a * y.dot(y) - by * by,
            a,
            bx,
            by,
        })
    }
}

pub(super) struct QuadricRow {
    /// `h = free − dx·(p·dx + lean)`.
    free: f32,
    lean: f32,
    by: f32,
}

impl Scan for Quadric {
    type Row = QuadricRow;

    fn bounds(&self) -> [f32; 4] {
        let (c, [rx, ry]) = (self.center, self.reach);
        [c.x - rx, c.y - ry, c.x + rx, c.y + ry]
    }

    fn row(&self, y: f32) -> Option<(f32, f32, QuadricRow)> {
        let dy = y - self.center.y;
        let (free, half) = (self.a - self.s * dy * dy, self.q * dy);
        let disc = half * half + self.p * free;
        // `p` — в единицах 1/px⁴: у большого тела он крошечный, но не ноль
        if disc < 0.0 || self.p <= 0.0 {
            return None;
        }
        let root = disc.sqrt();
        let row = QuadricRow {
            free,
            lean: 2.0 * half,
            by: self.by * dy,
        };
        let at = |edge: f32| self.center.x + (edge - half) / self.p;
        Some((at(-root), at(root), row))
    }

    fn depth(&self, row: &QuadricRow, x: f32, _: f32) -> Option<f32> {
        let dx = x - self.center.x;
        let h = row.free - dx * (self.p * dx + row.lean);
        (h >= 0.0).then(|| self.center.z - (self.bx * dx + row.by + h.sqrt()) / self.a)
    }
}

/// Оболочка двух шаров: они сами и усечённый конус между кругами касания.
pub(super) struct Cone {
    ends: [Vec3; 2],
    radii: [f32; 2],
    axis: Vec3,
    /// Квадрат длины оси на экране и самый толстый радиус: быстрый отсев.
    flat: f32,
    thick: f32,
    length: f32,
    taper: f32,
    span: f32,
    k2: f32,
}

impl Cone {
    /// `None` — один шар внутри другого: рисовать надо больший.
    fn new(ends: [Vec3; 2], radii: [f32; 2]) -> Option<Self> {
        let axis = ends[1] - ends[0];
        let (length, taper) = (axis.dot(axis), radii[0] - radii[1]);
        let span = length - taper * taper;
        (span > EPSILON * length.max(1.0)).then(|| Self {
            ends,
            radii,
            axis,
            flat: axis.x * axis.x + axis.y * axis.y,
            thick: radii[0].max(radii[1]),
            length,
            taper,
            span,
            k2: span - axis.z * axis.z,
        })
    }

    /// Ближняя точка шара на конце `end`.
    fn cap(&self, end: usize, x: f32, y: f32) -> Option<f32> {
        let (c, r) = (self.ends[end], self.radii[end]);
        let h = r * r - (x - c.x).powi(2) - (y - c.y).powi(2);
        (h >= 0.0).then(|| c.z - h.sqrt())
    }
}

fn nearest(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

impl Scan for Cone {
    /// Строка экрана.
    type Row = f32;

    fn bounds(&self) -> [f32; 4] {
        let ([a, b], [ra, rb]) = (self.ends, self.radii);
        [
            (a.x - ra).min(b.x - rb),
            (a.y - ra).min(b.y - rb),
            (a.x + ra).max(b.x + rb),
            (a.y + ra).max(b.y + rb),
        ]
    }

    fn row(&self, y: f32) -> Option<(f32, f32, f32)> {
        let [left, _, right, _] = self.bounds();
        Some((left, right, y))
    }

    fn depth(&self, y: &f32, x: f32, limit: f32) -> Option<f32> {
        let ([a, _], [ra, _], axis) = (self.ends, self.radii, self.axis);
        let (ox, oy) = (x - a.x, y - a.y);
        let flat = axis.x * ox + axis.y * oy;
        // Дальше самого толстого радиуса от оси на экране части нет
        let t = if self.flat > EPSILON {
            (flat / self.flat).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (px, py) = (ox - axis.x * t, oy - axis.y * t);
        if px * px + py * py > self.thick * self.thick {
            return None;
        }
        if a.z.min(self.ends[1].z) - self.thick >= limit {
            return None;
        }
        let caps = nearest(self.cap(0, x, *y), self.cap(1, x, *y));
        let (m1, m2) = (flat - axis.z * a.z, axis.z);
        let m5 = ox * ox + oy * oy + a.z * a.z;
        let k1 = -self.span * a.z - m1 * m2 + m2 * self.taper * ra;
        let k0 = self.span * m5 - m1 * m1 + 2.0 * m1 * self.taper * ra - self.length * ra * ra;
        // Только пояс между кругами касания
        let on_side = |z: f32| {
            let along = m1 - ra * self.taper + z * m2;
            (along > 0.0 && along < self.span).then_some(z)
        };
        let side = if self.k2.abs() <= EPSILON * self.span {
            (k1.abs() > EPSILON)
                .then(|| -k0 / (2.0 * k1))
                .and_then(on_side)
        } else {
            let h = k1 * k1 - k0 * self.k2;
            if h < 0.0 {
                None
            } else {
                let root = h.sqrt();
                nearest(
                    on_side((-k1 - root) / self.k2),
                    on_side((-k1 + root) / self.k2),
                )
            }
        };
        nearest(side, caps)
    }
}

/// Плоский эллипс: на экране тоже эллипс, с полуосями — проекциями своих.
pub(super) struct Disc {
    center: Vec3,
    axes: [Vec3; 2],
    /// Площадь проекции полуосей со знаком: с какой стороны диск виден.
    turn: f32,
}

impl Disc {
    /// `None` — диск ребром или изнанкой к зрителю; у части лицом к зрителю изнанки нет.
    fn new(center: Vec3, axes: [Vec3; 2], two_sided: bool) -> Option<Self> {
        let turn = axes[0].x * axes[1].y - axes[0].y * axes[1].x;
        let seen = if two_sided { turn.abs() } else { turn };
        (seen > EPSILON).then_some(Self { center, axes, turn })
    }
}

impl Scan for Disc {
    /// Строка экрана от центра.
    type Row = f32;

    fn bounds(&self) -> [f32; 4] {
        let (c, [u, v]) = (self.center, self.axes);
        let (rx, ry) = (u.x.hypot(v.x), u.y.hypot(v.y));
        [c.x - rx, c.y - ry, c.x + rx, c.y + ry]
    }

    fn row(&self, y: f32) -> Option<(f32, f32, f32)> {
        let [left, _, right, _] = self.bounds();
        Some((left, right, y - self.center.y))
    }

    fn depth(&self, dy: &f32, x: f32, _: f32) -> Option<f32> {
        let (dx, [u, v]) = (x - self.center.x, self.axes);
        let along = (dx * v.y - dy * v.x) / self.turn;
        let across = (dy * u.x - dx * u.y) / self.turn;
        let depth = self.center.z + along * u.z + across * v.z;
        (along * along + across * across <= 1.0).then_some(depth)
    }
}

pub(super) enum Prim {
    Ball(Ball),
    Quadric(Quadric),
    Cone(Cone),
    Disc(Disc),
}

impl Prim {
    fn ball(center: Vec3, radius: f32) -> Self {
        Self::Ball(Ball { center, radius })
    }

    fn capsule(ends: [Vec3; 2], radii: [f32; 2]) -> Self {
        match Cone::new(ends, radii) {
            Some(cone) => Self::Cone(cone),
            None if radii[0] >= radii[1] => Self::ball(ends[0], radii[0]),
            None => Self::ball(ends[1], radii[1]),
        }
    }

    /// Охват на экране: лево, верх, право, низ.
    pub fn bounds(&self) -> [f32; 4] {
        match self {
            Self::Ball(ball) => ball.bounds(),
            Self::Quadric(quadric) => quadric.bounds(),
            Self::Cone(cone) => cone.bounds(),
            Self::Disc(disc) => disc.bounds(),
        }
    }

    /// То же, но толще на `by` во все стороны: из-под части выглядывает каймой.
    pub fn grown(&self, by: f32) -> Option<Self> {
        let longer = |axis: Vec3| axis * (1.0 + by / axis.length().max(EPSILON));
        match self {
            Self::Ball(ball) => Some(Self::ball(ball.center, ball.radius + by)),
            Self::Quadric(quadric) => {
                Quadric::new(quadric.center, quadric.axes.map(longer)).map(Self::Quadric)
            }
            Self::Cone(cone) => Some(Self::capsule(cone.ends, cone.radii.map(|r| r + by))),
            Self::Disc(disc) => Disc::new(disc.center, disc.axes.map(longer), true).map(Self::Disc),
        }
    }

    /// Части скелета глазами камеры: цепочка даёт по капсуле на звено, у полосатой —
    /// со своим цветом.
    pub fn each(part: &Part, joints: &[Joint], mut each: impl FnMut(Self, Option<u32>)) {
        let Some(joint) = joints.get(part.bone) else {
            return;
        };
        let (space, size) = (Space::of(part, joint), joint.size);
        let mut emit = |prim: Self| each(prim, None);
        match &part.shape {
            Shape::Ball { at, radius } => emit(Self::ball(space.point(*at), radius * size)),
            Shape::Ellipsoid { at, radii } => {
                let axes = [
                    space.vector(Vec3::new(radii[0], 0.0, 0.0)),
                    space.vector(Vec3::new(0.0, radii[1], 0.0)),
                    space.vector(Vec3::new(0.0, 0.0, radii[2])),
                ];
                if let Some(quadric) = Quadric::new(space.point(*at), axes) {
                    emit(Self::Quadric(quadric));
                }
            }
            Shape::Capsule { from, to, radii } => emit(Self::capsule(
                [space.point(*from), space.point(*to)],
                radii.map(|r| r * size),
            )),
            Shape::Disc { at, axes } => {
                let axes = axes.map(|axis| space.vector(axis));
                if let Some(disc) = Disc::new(space.point(*at), axes, part.facing.is_some()) {
                    emit(Self::Disc(disc));
                }
            }
            Shape::Chain { .. } => chain(joint, &part.shape, joints, each),
        }
    }
}

/// Узлов у цепочки не больше: костей или звеньев дуги, с началом.
const MAX_KNOTS: usize = 17;

/// Звенья от точки кости через начала костей `through` или по дуге, для которой те —
/// опорные точки. Размер последней кости против первой — насколько цепочка вытянута:
/// поджатая лапа уходит в тело и тает.
fn chain(joint: &Joint, shape: &Shape, joints: &[Joint], mut emit: impl FnMut(Prim, Option<u32>)) {
    let Shape::Chain {
        from,
        through,
        radii,
        reach,
        curve,
        bands,
        floor,
    } = shape
    else {
        return;
    };
    let (Some(last), true) = (
        through.last().and_then(|bone| joints.get(*bone)),
        joint.size > 0.0,
    ) else {
        return;
    };
    let out = (last.size / joint.size).clamp(0.0, 1.0);
    let mut knots = [joint.point(*from); MAX_KNOTS];
    let mut count = 1;
    for bone in through.iter().take(MAX_KNOTS - 1) {
        let Some(at) = joints.get(*bone) else {
            return;
        };
        knots[count] = at.origin;
        count += 1;
    }
    if *floor {
        for knot in &mut knots[..count] {
            knot.z = knot.z.max(0.0);
        }
    }
    if let (Some(links), 3) = (*curve, count) {
        let [start, bend, end] = [knots[0], knots[1], knots[2]];
        count = links.min(MAX_KNOTS - 1) + 1;
        for (index, knot) in knots[..count].iter_mut().enumerate() {
            let t = index as f32 / (count - 1) as f32;
            *knot = start.lerp(bend, t).lerp(bend.lerp(end, t), t);
        }
    }
    let knots = &knots[..count];
    let length: f32 = knots
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).length())
        .sum();
    let slack = reach.map_or(1.0, |reach| {
        (reach * joint.size / length.max(EPSILON)).min(1.0)
    });
    // Поджатая наполовину короче, но не тоньше
    let (pull, thick) = (out * slack, joint.size * (2.0 * out).min(1.0));
    if pull <= 0.0 || thick <= 0.0 {
        return;
    }
    let links = count - 1;
    // Радиусы заданы через равные доли длины
    let radius = |index: usize| {
        let at = index as f32 / links as f32 * (radii.len() - 1) as f32;
        let low = (at as usize).min(radii.len() - 2);
        (radii[low] + (radii[low + 1] - radii[low]) * (at - low as f32)) * thick
    };
    let place = |knot: Vec3| view(knots[0].lerp(knot, pull));
    // От конца к началу: на стыке колец видно то, что дальше от тела
    for index in (0..links).rev() {
        // Цвета считаются от конца: кончик всегда первого цвета
        let band = (!bands.is_empty()).then(|| bands[(links - 1 - index) % bands.len()]);
        let ends = [place(knots[index]), place(knots[index + 1])];
        emit(
            Prim::capsule(ends, [radius(index), radius(index + 1)]),
            band,
        );
    }
}

/// Куда ложатся координаты формы: в оси кости или, у части лицом к зрителю, в оси экрана
/// от точки привязки.
struct Space<'a> {
    joint: &'a Joint,
    anchor: Option<Vec3>,
    /// На столько точки ближе к зрителю, чем стоят.
    lift: f32,
}

impl<'a> Space<'a> {
    fn of(part: &Part, joint: &'a Joint) -> Self {
        let turned = view(joint.frame.forward).z;
        // С дальней стороны кости или с затылка часть целиком за костью: не «сквозь» неё
        let push = |offset: Vec3| {
            let far = offset.z > FAR_SIDE * offset.length();
            let hidden = part.behind && (far || turned > TURNED_AWAY);
            if hidden {
                BEHIND * offset.length()
            } else {
                0.0
            }
        };
        let anchor = part.facing.map(|at| {
            // Отвернувшись, кость прячет привязку за собой
            let offset = view(joint.vector(at * (1.0 - part.sink * turned.clamp(0.0, 1.0))));
            view(joint.origin) + offset + Vec3::new(0.0, 0.0, push(offset))
        });
        // Обычная часть уходит за кость по своей первой точке
        let sunk = match (&part.facing, &part.shape) {
            (
                None,
                Shape::Ball { at, .. } | Shape::Ellipsoid { at, .. } | Shape::Disc { at, .. },
            ) => push(view(joint.vector(*at))),
            (None, Shape::Capsule { from, .. }) => push(view(joint.vector(*from))),
            _ => 0.0,
        };
        Self {
            joint,
            anchor,
            lift: part.lift * joint.size - sunk,
        }
    }

    fn vector(&self, local: Vec3) -> Vec3 {
        if self.anchor.is_none() {
            return view(self.joint.vector(local));
        }
        let ([x, y, z], size) = (self.joint.stretch, self.joint.size);
        // Вправо, вверх по экрану, к зрителю
        Vec3::new(local.x * x, -local.y * y, -local.z * z) * size
    }

    fn point(&self, local: Vec3) -> Vec3 {
        let at = match self.anchor {
            Some(anchor) => anchor + self.vector(local),
            None => view(self.joint.point(local)),
        };
        Vec3::new(at.x, at.y, at.z - self.lift)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn depth_at<S: Scan>(scan: &S, x: f32, y: f32) -> Option<f32> {
        let (left, right, row) = scan.row(y)?;
        (left..=right)
            .contains(&x)
            .then(|| scan.depth(&row, x, f32::INFINITY))?
    }

    #[test]
    fn ball_is_a_circle_and_nearer_in_the_middle() {
        let ball = Ball {
            center: Vec3::new(2.0, 3.0, 10.0),
            radius: 5.0,
        };
        assert_eq!(depth_at(&ball, 2.0, 3.0), Some(5.0));
        assert!(depth_at(&ball, 6.9, 3.0).unwrap() > 9.0);
        assert!(depth_at(&ball, 7.1, 3.0).is_none());
        assert!(depth_at(&ball, 2.0, 8.1).is_none());
        // Заслонён целиком — не считается, выглядывает — считается
        let (_, _, row) = ball.row(3.0).unwrap();
        assert!(ball.depth(&row, 2.0, 4.0).is_none());
        assert!(ball.depth(&row, 2.0, 6.0).is_some());
    }

    #[test]
    fn quadric_with_equal_axes_is_a_ball_and_its_span_is_exact() {
        let center = Vec3::new(3.0, -2.0, 9.0);
        let ball = Ball {
            center,
            radius: 6.0,
        };
        let frame = crate::rig::Frame::new(0.7, 0.4, -0.3);
        let axes = [frame.forward * 6.0, frame.right * 6.0, frame.up * 6.0];
        let quadric = Quadric::new(center, axes).unwrap();
        for step in -40..40 {
            let (x, y) = (center.x + step as f32 * 0.2, center.y + 1.5);
            match (depth_at(&ball, x, y), depth_at(&quadric, x, y)) {
                (Some(a), Some(b)) => assert!((a - b).abs() < 1e-3, "{a} {b}"),
                (a, b) => assert_eq!(a.is_some(), b.is_some(), "{x}"),
            }
        }
        let (left, right, _) = quadric.row(center.y).unwrap();
        assert!((left - (center.x - 6.0)).abs() < 1e-3 && (right - (center.x + 6.0)).abs() < 1e-3);
        assert!(Quadric::new(center, [axes[0], axes[0], axes[2]]).is_none());
    }

    #[test]
    fn stretched_quadric_keeps_its_far_point_on_the_long_axis() {
        let axes = [
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 6.0),
        ];
        let quadric = Quadric::new(Vec3::ZERO, axes).unwrap();
        assert_eq!(quadric.bounds(), [-10.0, -4.0, 10.0, 4.0]);
        // Большое тело на крупном экране — тот же эллипсоид
        let large = Quadric::new(Vec3::ZERO, axes.map(|axis| axis * 40.0)).unwrap();
        assert!((depth_at(&large, 0.0, 0.0).unwrap() + 240.0).abs() < 1e-2);
        assert!(depth_at(&large, 396.0, 0.0).is_some());
        assert!((depth_at(&quadric, 0.0, 0.0).unwrap() + 6.0).abs() < 1e-4);
        assert!(depth_at(&quadric, 9.9, 0.0).is_some());
        assert!(depth_at(&quadric, 0.0, 4.1).is_none());
    }

    #[test]
    fn disc_shows_one_side_unless_it_faces_the_viewer() {
        let axes = [Vec3::new(4.0, 0.0, 1.0), Vec3::new(0.0, 3.0, 0.0)];
        let flipped = [axes[1], axes[0]];
        let disc = Disc::new(Vec3::ZERO, axes, false).unwrap();
        assert!((depth_at(&disc, 2.0, 0.0).unwrap() - 0.5).abs() < 1e-5);
        assert!(depth_at(&disc, 4.1, 0.0).is_none());
        assert!(Disc::new(Vec3::ZERO, flipped, false).is_none());
        assert!(Disc::new(Vec3::ZERO, flipped, true).is_some());
        let edge_on = [Vec3::new(0.0, 0.0, 4.0), Vec3::new(0.0, 3.0, 0.0)];
        assert!(Disc::new(Vec3::ZERO, edge_on, true).is_none());
    }

    /// Точка внутри оболочки, если она внутри одного из промежуточных шаров.
    fn inside_hull(p: Vec3, [a, b]: [Vec3; 2], [ra, rb]: [f32; 2]) -> bool {
        (0..=60).any(|i| {
            let t = i as f32 / 60.0;
            (p - a.lerp(b, t)).length() <= ra + (rb - ra) * t
        })
    }

    #[test]
    fn cone_matches_the_hull_of_its_balls_from_any_side() {
        let mut rng = Rng::new(11);
        let mut signed = |reach: f32| rng.range(-reach, reach);
        let (mut hits, step) = (0, 0.04);
        for _ in 0..200 {
            let ends = [
                Vec3::new(signed(8.0), signed(8.0), signed(8.0)),
                Vec3::new(signed(8.0), signed(8.0), signed(8.0)),
            ];
            let radii = [signed(2.0) + 2.5, signed(2.0) + 2.5];
            let (x, y) = (signed(6.0), signed(6.0));
            let marched = (0..2000)
                .map(|i| i as f32 * step - 40.0)
                .find(|z| inside_hull(Vec3::new(x, y, *z), ends, radii));
            let hit = match Prim::capsule(ends, radii) {
                Prim::Cone(cone) => depth_at(&cone, x, y),
                Prim::Ball(ball) => depth_at(&ball, x, y),
                _ => unreachable!(),
            };
            match (marched, hit) {
                (Some(marched), Some(hit)) => {
                    hits += 1;
                    assert!((marched - hit).abs() < 0.15, "{ends:?} {radii:?} {x} {y}");
                }
                // Касание по краю маршем можно проскочить, а пропустить тело — нельзя
                (Some(marched), None) => {
                    let deep = inside_hull(Vec3::new(x, y, marched + 0.5), ends, radii);
                    assert!(!deep, "{ends:?} {radii:?} {x} {y}");
                }
                (None, Some(hit)) => {
                    let grazed = inside_hull(Vec3::new(x, y, hit + 0.3), ends, radii);
                    assert!(!grazed, "{ends:?} {radii:?} {x} {y}");
                }
                (None, None) => {}
            }
        }
        assert!(hits > 50);
    }
}
