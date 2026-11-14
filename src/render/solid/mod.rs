//! Тело из частей скелета с буфером глубины: что ближе к зрителю, то и видно — порядок
//! слоёв не задаётся. Камера одна — взгляд сверху под 30° (`rig::view`).

mod prim;
#[cfg(test)]
mod tests;

use std::ops::Range;

use self::prim::{Prim, Scan};
use super::Canvas;
use crate::math::Vec2;
use crate::rig::{Skeleton, World};

/// Отсчётов на пиксель: четыре подстроки, в каждой свой сдвиг по `x` — повёрнутая
/// решётка. Края, и внешние, и между частями, сглажены одинаково.
const SUB: usize = 4;
const OFFSET: [f32; SUB] = [0.375, 0.875, 0.125, 0.625];
/// Старший байт закрытого отсчёта: обычная часть; у части с каймой — `PLAIN` минус номер
/// обводки с единицы.
const PLAIN: u32 = 0xFF00_0000;
const FLAG_SHIFT: u32 = 24;

fn rimmed(outline: usize) -> u32 {
    PLAIN - ((outline as u32 + 1) << FLAG_SHIFT)
}

/// Буферы живут между кадрами и покрывают только тело, а не весь холст.
#[derive(Default)]
pub struct Solid {
    /// Пиксели холста под буфером: начало по `x` и `y`, ширина и число подстрок.
    left: usize,
    top: usize,
    width: usize,
    rows: usize,
    depth: Vec<f32>,
    /// Старший байт — закрыт ли отсчёт и чьей каймой, остальное — `0xRRGGBB`.
    color: Vec<u32>,
    /// Части кадра глазами камеры и чем их рисовать.
    prims: Vec<(Prim, Paint)>,
    /// Части с каймой, утолщённые на её ширину: рисуются последними, под всё с каймой.
    rims: Vec<(Prim, Paint)>,
}

/// Что отсчёт делает с частью, оказавшейся ближе нарисованного.
#[derive(Clone, Copy)]
enum Paint {
    Fill(u32),
    /// Полупрозрачное только красит то, что под ним.
    Tint(u32, f32),
    /// Кайма: ложится на пустое и на то, что дальше, но не на части своей обводки —
    /// швов нет. Второе число — их старший байт.
    Edge(u32, u32),
}

/// Индекс не дальше `limit`; мусор на входе — ноль.
fn index(at: f32, limit: usize) -> usize {
    at.clamp(0.0, limit as f32) as usize
}

fn mix(under: u32, over: u32, opacity: f32) -> u32 {
    let share = (opacity * 256.0) as u32;
    let channel = |shift: u32| {
        let (a, b) = (under >> shift & 255, over >> shift & 255);
        (a * (256 - share) + b * share) >> 8 << shift
    };
    under & PLAIN | channel(16) | channel(8) | channel(0)
}

impl Solid {
    /// `anchor` — точка на земле под телом в координатах холста; `controls` — регуляторы
    /// скелета по порядку, недостающие — нули. Возвращает нарисованные пиксели по `x` и `y`.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        anchor: Vec2,
        skeleton: &Skeleton,
        world: &World,
        controls: &[f32],
    ) -> [Range<usize>; 2] {
        self.collect(skeleton, world, controls);
        if !self.frame(canvas, anchor) {
            return [0..0, 0..0];
        }
        // Буфер начинается не в углу холста: точка на земле — от его угла
        let anchor = anchor - Vec2::new(self.left as f32, self.top as f32);
        // Списки забираются на время прохода и возвращаются: память не выделяется заново
        let (prims, rims) = (
            std::mem::take(&mut self.prims),
            std::mem::take(&mut self.rims),
        );
        for (prim, paint) in prims.iter().chain(&rims) {
            match prim {
                Prim::Ball(ball) => self.scan(anchor, ball, *paint),
                Prim::Quadric(quadric) => self.scan(anchor, quadric, *paint),
                Prim::Cone(cone) => self.scan(anchor, cone, *paint),
                Prim::Disc(disc) => self.scan(anchor, disc, *paint),
            }
        }
        (self.prims, self.rims) = (prims, rims);
        self.resolve(canvas)
    }

    /// Части кадра и их кайма.
    fn collect(&mut self, skeleton: &Skeleton, world: &World, controls: &[f32]) {
        self.prims.clear();
        self.rims.clear();
        let control = |index: &usize| controls.get(*index).copied().unwrap_or(0.0);
        for part in skeleton.parts() {
            let shown: f32 = part.show.iter().map(control).product();
            let hidden: f32 = part.hide.iter().map(|index| 1.0 - control(index)).product();
            let opacity = (shown * hidden).clamp(0.0, 1.0);
            if opacity.is_nan() || opacity <= 0.0 {
                continue;
            }
            let rim = part.rim.filter(|_| opacity >= 1.0);
            let outline = rim.and_then(|rim| Some((rim, *skeleton.outlines().get(rim)?)));
            let size = world
                .joints()
                .get(part.bone)
                .map_or(0.0, |joint| joint.size);
            Prim::each(part, world.joints(), |prim, band| {
                let color = band.unwrap_or(part.color);
                let paint = match (outline, opacity >= 1.0) {
                    (Some((rim, _)), _) => Paint::Fill(rimmed(rim) | color),
                    (None, true) => Paint::Fill(PLAIN | color),
                    (None, false) => Paint::Tint(color, opacity),
                };
                if let Some((rim, outline)) = outline {
                    let edge = Paint::Edge(PLAIN | outline.color, rimmed(rim));
                    let grown = prim.grown(outline.width * size);
                    self.rims.extend(grown.map(|grown| (grown, edge)));
                }
                self.prims.push((prim, paint));
            });
        }
    }

    /// Буфер под всё, что рисует кадр, в пределах холста; `false` — рисовать нечего.
    fn frame(&mut self, canvas: &Canvas, anchor: Vec2) -> bool {
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for (prim, _) in self.prims.iter().chain(&self.rims) {
            let [l, t, r, b] = prim.bounds();
            if [l, t, r, b].iter().all(|v| v.is_finite()) {
                bounds = [
                    bounds[0].min(l),
                    bounds[1].min(t),
                    bounds[2].max(r),
                    bounds[3].max(b),
                ];
            }
        }
        let (wide, tall) = (canvas.width() as usize, canvas.height() as usize);
        let xs =
            index((bounds[0] + anchor.x).floor(), wide)..index((bounds[2] + anchor.x).ceil(), wide);
        let ys =
            index((bounds[1] + anchor.y).floor(), tall)..index((bounds[3] + anchor.y).ceil(), tall);
        if xs.is_empty() || ys.is_empty() {
            return false;
        }
        (self.left, self.top) = (xs.start, ys.start);
        (self.width, self.rows) = (xs.len(), ys.len() * SUB);
        let len = self.width * self.rows;
        self.depth.clear();
        self.depth.resize(len, f32::INFINITY);
        self.color.clear();
        self.color.resize(len, 0);
        true
    }

    fn scan<S: Scan>(&mut self, anchor: Vec2, part: &S, paint: Paint) {
        let [_, top, _, bottom] = part.bounds();
        let at = |y: f32| (y + anchor.y) * SUB as f32 - 0.5;
        let js = index(at(top).ceil(), self.rows)..index(at(bottom).floor() + 1.0, self.rows);
        for j in js {
            let y = (j as f32 + 0.5) / SUB as f32 - anchor.y;
            let Some((left, right, row)) = part.row(y) else {
                continue;
            };
            // `x` отсчёта — индекс плюс сдвиг
            let shift = OFFSET[j % SUB] - anchor.x;
            let xs = index((left - shift).ceil(), self.width)
                ..index((right - shift).floor() + 1.0, self.width);
            let base = j * self.width;
            let depths = &mut self.depth[base..][xs.clone()];
            let colors = &mut self.color[base..][xs.clone()];
            for (offset, (seen, color)) in depths.iter_mut().zip(colors).enumerate() {
                let x = (xs.start + offset) as f32 + shift;
                let Some(depth) = part.depth(&row, x, *seen) else {
                    continue;
                };
                if depth >= *seen {
                    continue;
                }
                match paint {
                    Paint::Fill(tag) => (*seen, *color) = (depth, tag),
                    Paint::Tint(tag, opacity) if *color != 0 => *color = mix(*color, tag, opacity),
                    Paint::Edge(tag, own) if *color & PLAIN != own => {
                        (*seen, *color) = (depth, tag)
                    }
                    Paint::Tint(..) | Paint::Edge(..) => {}
                }
            }
        }
    }

    /// Среднее отсчётов пикселя: закрытая доля — его непрозрачность.
    fn resolve(&self, canvas: &mut Canvas) -> [Range<usize>; 2] {
        let xs = self.left..self.left + self.width;
        let ys = self.top..self.top + self.rows / SUB;
        canvas.draw(xs.clone(), ys.clone(), |x, y| {
            let mut sum = [0u32; 4];
            for sub in 0..SUB {
                let row = (y - self.top) * SUB + sub;
                let sample = self.color[row * self.width + x - self.left];
                if sample == 0 {
                    continue;
                }
                for (sum, byte) in sum.iter_mut().zip((sample | PLAIN).to_be_bytes()) {
                    *sum += u32::from(byte);
                }
            }
            // Незакрытые отсчёты — нули, поэтому среднее уже premultiplied
            let total = SUB as u32;
            u32::from_be_bytes(sum.map(|channel| ((channel + total / 2) / total) as u8))
        });
        [xs, ys]
    }
}
