//! Программный холст: premultiplied `0xAARRGGBB`, целочисленное смешивание.

use std::ops::Range;

use crate::math::{Rect, Vec2};

/// Прямой (не premultiplied) `0xRRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

/// Кадр на листе: `rect` = x, y, w, h в пикселях листа, пиксели premultiplied.
#[derive(Clone, Copy)]
pub struct Sprite<'a> {
    pub pixels: &'a [u32],
    pub stride: usize,
    pub rect: [u32; 4],
}

pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    stride: usize,
    width: u32,
    height: u32,
}

fn div255(x: u32) -> u32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

fn alpha8(opacity: f32) -> u32 {
    if opacity.is_nan() {
        0
    } else {
        (opacity.clamp(0.0, 1.0) * 255.0 + 0.5) as u32
    }
}

fn map_channels(pixel: u32, f: impl Fn(u32) -> u32) -> u32 {
    f(pixel >> 24) << 24 | f(pixel >> 16 & 255) << 16 | f(pixel >> 8 & 255) << 8 | f(pixel & 255)
}

fn over(dst: u32, src: u32) -> u32 {
    let inverse = 255 - (src >> 24);
    let scaled = map_channels(dst, |c| div255(c * inverse));
    // Каналы src ≤ его альфы, поэтому суммы не переполняют байт.
    src + scaled
}

fn shade(color: Color, alpha: u32) -> u32 {
    let [_, r, g, b] = color.0.to_be_bytes().map(u32::from);
    alpha << 24 | div255(r * alpha) << 16 | div255(g * alpha) << 8 | div255(b * alpha)
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Индексы пикселей, чьи центры лежат в `[lo, hi)`, обрезанные по `limit`.
fn span(lo: f32, hi: f32, limit: u32) -> Range<usize> {
    let start = ((lo - 0.5).ceil() as i64).clamp(0, i64::from(limit));
    let end = ((hi - 0.5).ceil() as i64).clamp(start, i64::from(limit));
    start as usize..end as usize
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u32], stride: usize, width: u32, height: u32) -> Option<Self> {
        let (w, h) = (width as usize, height as usize);
        let needed = match h {
            0 => 0,
            _ => (h - 1).checked_mul(stride)?.checked_add(w)?,
        };
        (stride >= w && pixels.len() >= needed).then_some(Self {
            pixels,
            stride,
            width,
            height,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn clear(&mut self) {
        for y in 0..self.height as usize {
            let row = y * self.stride;
            self.pixels[row..row + self.width as usize].fill(0);
        }
    }

    /// Стирает всё, чей центр вне `keep` (координаты холста): персонаж за краем экрана.
    pub fn clear_outside(&mut self, keep: Rect) {
        let xs = span(keep.min.x, keep.max.x, self.width);
        let ys = span(keep.min.y, keep.max.y, self.height);
        for y in 0..self.height as usize {
            let row = &mut self.pixels[y * self.stride..y * self.stride + self.width as usize];
            if !ys.contains(&y) {
                row.fill(0);
                continue;
            }
            row[..xs.start].fill(0);
            row[xs.end..].fill(0);
        }
    }

    /// Мозаика: каждый квадрат `block`×`block` тянется к своему среднему цвету
    /// на долю `strength` (1 — сплошная заливка).
    /// `phase` — сдвиг сетки, чтобы она была привязана к миру, а не к холсту.
    pub fn pixelate(&mut self, block: u32, phase: [u32; 2], strength: f32) {
        let mix = alpha8(strength);
        if block < 2 || mix == 0 {
            return;
        }
        let (width, height, block) = (self.width as usize, self.height as usize, block as usize);
        let first = |phase: u32| (block - phase as usize % block) % block;
        let starts = |first: usize, limit: usize| {
            let lead = (first > 0).then_some(0);
            lead.into_iter().chain((first..limit).step_by(block))
        };
        let (first_x, first_y) = (first(phase[0]), first(phase[1]));
        for y0 in starts(first_y, height) {
            let y1 = if y0 < first_y { first_y } else { y0 + block }.min(height);
            for x0 in starts(first_x, width) {
                let x1 = if x0 < first_x { first_x } else { x0 + block }.min(width);
                self.flatten_block(x0..x1, y0..y1, mix);
            }
        }
    }

    fn flatten_block(&mut self, xs: Range<usize>, ys: Range<usize>, mix: u32) {
        let count = (xs.len() * ys.len()) as u32;
        let mut sum = [0u32; 4];
        for y in ys.clone() {
            for pixel in &self.pixels[y * self.stride..][xs.clone()] {
                for (total, byte) in sum.iter_mut().zip(pixel.to_be_bytes()) {
                    *total += u32::from(byte);
                }
            }
        }
        if count == 0 || sum[0] == 0 {
            return;
        }
        // Среднее premultiplied-каналов остаётся premultiplied
        let mean = sum.map(|total| (total + count / 2) / count);
        for y in ys {
            for pixel in &mut self.pixels[y * self.stride..][xs.clone()] {
                let mut bytes = pixel.to_be_bytes();
                for (byte, mean) in bytes.iter_mut().zip(mean) {
                    *byte = div255(u32::from(*byte) * (255 - mix) + mean * mix) as u8;
                }
                *pixel = u32::from_be_bytes(bytes);
            }
        }
    }

    pub fn alpha_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        (self.pixels[y as usize * self.stride + x as usize] >> 24) as u8
    }

    fn draw(
        &mut self,
        xs: Range<usize>,
        ys: Range<usize>,
        mut source: impl FnMut(usize, usize) -> u32,
    ) {
        for y in ys {
            let row = y * self.stride;
            for x in xs.clone() {
                let src = source(x, y);
                if src >> 24 != 0 {
                    let dst = &mut self.pixels[row + x];
                    *dst = over(*dst, src);
                }
            }
        }
    }

    pub fn blit(
        &mut self,
        sprite: Sprite<'_>,
        top_left: Vec2,
        scale: f32,
        flip_x: bool,
        opacity: f32,
    ) {
        let [sx, sy, sw, sh] = sprite.rect.map(|v| v as usize);
        let in_sheet = sx
            .checked_add(sw)
            .is_some_and(|right| right <= sprite.stride)
            && sy
                .checked_add(sh)
                .and_then(|bottom| bottom.checked_sub(1))
                .and_then(|last| last.checked_mul(sprite.stride))
                .and_then(|row| row.checked_add(sx + sw))
                .is_some_and(|end| end <= sprite.pixels.len());
        let opacity = alpha8(opacity);
        if sw == 0 || sh == 0 || !in_sheet || opacity == 0 {
            return;
        }
        if !finite(&[top_left.x, top_left.y, scale]) || scale <= 0.0 {
            return;
        }
        let xs = span(top_left.x, top_left.x + sw as f32 * scale, self.width);
        let ys = span(top_left.y, top_left.y + sh as f32 * scale, self.height);
        let inverse = 1.0 / scale;
        self.draw(xs, ys, |x, y| {
            let u = (((x as f32 + 0.5 - top_left.x) * inverse) as usize).min(sw - 1);
            let v = (((y as f32 + 0.5 - top_left.y) * inverse) as usize).min(sh - 1);
            let u = if flip_x { sw - 1 - u } else { u };
            let pixel = sprite.pixels[(sy + v) * sprite.stride + sx + u];
            if opacity == 255 {
                pixel
            } else {
                map_channels(pixel, |c| div255(c * opacity))
            }
        });
    }

    pub fn fill_circle(&mut self, center: Vec2, radius: f32, color: Color, opacity: f32) {
        let opacity = alpha8(opacity);
        if !finite(&[center.x, center.y, radius]) || radius <= 0.0 || opacity == 0 {
            return;
        }
        let reach = radius + 0.5;
        let xs = span(center.x - reach, center.x + reach, self.width);
        let ys = span(center.y - reach, center.y + reach, self.height);
        self.draw(xs, ys, |x, y| {
            let distance = (x as f32 + 0.5 - center.x).hypot(y as f32 + 0.5 - center.y);
            shade(color, coverage(radius + 0.5 - distance, opacity))
        });
    }

    /// `rotation` — поворот большой оси `radii.x` по часовой стрелке на экране, рад.
    pub fn fill_ellipse(
        &mut self,
        center: Vec2,
        radii: Vec2,
        rotation: f32,
        color: Color,
        opacity: f32,
    ) {
        let opacity = alpha8(opacity);
        let ok = finite(&[center.x, center.y, radii.x, radii.y, rotation]);
        if !ok || radii.x <= 0.0 || radii.y <= 0.0 || opacity == 0 {
            return;
        }
        let (sin, cos) = rotation.sin_cos();
        let reach = Vec2::new(
            (radii.x * cos).hypot(radii.y * sin),
            (radii.x * sin).hypot(radii.y * cos),
        ) + Vec2::new(0.5, 0.5);
        let xs = span(center.x - reach.x, center.x + reach.x, self.width);
        let ys = span(center.y - reach.y, center.y + reach.y, self.height);
        self.draw(xs, ys, |x, y| {
            let (dx, dy) = (x as f32 + 0.5 - center.x, y as f32 + 0.5 - center.y);
            let local = Vec2::new(dx * cos + dy * sin, dy * cos - dx * sin);
            let scaled = Vec2::new(local.x / radii.x, local.y / radii.y);
            let k0 = scaled.length();
            let k1 = Vec2::new(scaled.x / radii.x, scaled.y / radii.y).length();
            // Приближённое расстояние до края в пикселях
            let distance = if k1 > f32::EPSILON {
                k0 * (k0 - 1.0) / k1
            } else {
                -radii.x.min(radii.y)
            };
            shade(color, coverage(0.5 - distance, opacity))
        });
    }

    /// Отрезок с круглыми концами; радиусы на концах разные.
    pub fn fill_capsule(
        &mut self,
        a: Vec2,
        b: Vec2,
        radius_a: f32,
        radius_b: f32,
        color: Color,
        opacity: f32,
    ) {
        let opacity = alpha8(opacity);
        let ok = finite(&[a.x, a.y, b.x, b.y, radius_a, radius_b]);
        if !ok || radius_a < 0.0 || radius_b < 0.0 || radius_a.max(radius_b) <= 0.0 || opacity == 0
        {
            return;
        }
        let reach = Vec2::new(0.5, 0.5);
        let lo = Vec2::new(
            (a.x - radius_a).min(b.x - radius_b),
            (a.y - radius_a).min(b.y - radius_b),
        );
        let hi = Vec2::new(
            (a.x + radius_a).max(b.x + radius_b),
            (a.y + radius_a).max(b.y + radius_b),
        );
        let xs = span(lo.x - reach.x, hi.x + reach.x, self.width);
        let ys = span(lo.y - reach.y, hi.y + reach.y, self.height);
        let along = b - a;
        let length_sq = along.dot(along);
        self.draw(xs, ys, |x, y| {
            let from_a = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - a;
            let t = if length_sq > f32::EPSILON {
                (from_a.dot(along) / length_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let radius = radius_a + (radius_b - radius_a) * t;
            let distance = (from_a - along * t).length() - radius;
            shade(color, coverage(0.5 - distance, opacity))
        });
    }

    pub fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color, opacity: f32) {
        let opacity = alpha8(opacity);
        let (w, h) = (rect.width(), rect.height());
        if !finite(&[rect.min.x, rect.min.y, rect.max.x, rect.max.y, radius])
            || w <= 0.0
            || h <= 0.0
            || opacity == 0
        {
            return;
        }
        let half = Vec2::new(w * 0.5, h * 0.5);
        let center = rect.center();
        let radius = radius.clamp(0.0, half.x.min(half.y));
        let xs = span(rect.min.x - 0.5, rect.max.x + 0.5, self.width);
        let ys = span(rect.min.y - 0.5, rect.max.y + 0.5, self.height);
        self.draw(xs, ys, |x, y| {
            let qx = (x as f32 + 0.5 - center.x).abs() - (half.x - radius);
            let qy = (y as f32 + 0.5 - center.y).abs() - (half.y - radius);
            let outside = qx.max(0.0).hypot(qy.max(0.0));
            let distance = outside + qx.max(qy).min(0.0) - radius;
            shade(color, coverage(0.5 - distance, opacity))
        });
    }

    pub fn blit_mask(
        &mut self,
        coverage: &[u8],
        mask_width: u32,
        mask_height: u32,
        top_left: Vec2,
        color: Color,
        opacity: f32,
    ) {
        let (mw, mh) = (mask_width as usize, mask_height as usize);
        let opacity = alpha8(opacity);
        if mw == 0
            || mh == 0
            || mw.checked_mul(mh).is_none_or(|len| coverage.len() < len)
            || !finite(&[top_left.x, top_left.y])
            || opacity == 0
        {
            return;
        }
        // Маска ложится на целые пиксели.
        let (left, top) = (top_left.x.round(), top_left.y.round());
        let xs = span(left, left + mw as f32, self.width);
        let ys = span(top, top + mh as f32, self.height);
        self.draw(xs, ys, |x, y| {
            let u = (x as f32 - left) as usize;
            let v = (y as f32 - top) as usize;
            shade(color, div255(u32::from(coverage[v * mw + u]) * opacity))
        });
    }
}

/// Доля покрытия `edge` (в пикселях) с учётом прозрачности, байт 0..=255.
fn coverage(edge: f32, opacity: u32) -> u32 {
    div255((edge.clamp(0.0, 1.0) * 255.0 + 0.5) as u32 * opacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: u32 = 0xFF_FF_00_00;

    fn canvas(buffer: &mut [u32], side: u32) -> Canvas<'_> {
        Canvas::new(buffer, side as usize, side, side).unwrap()
    }

    fn sheet(width: usize, height: usize, pixel: u32) -> Vec<u32> {
        vec![pixel; width * height]
    }

    #[test]
    fn pixelate_averages_blocks_on_a_shifted_grid() {
        let mut buffer = vec![0u32; 36];
        buffer[7] = RED;
        let mut c = canvas(&mut buffer, 6);
        // Сдвиг 1: блоки начинаются с x, y = 1, 3, 5, а у края остаётся полоса в пиксель
        c.pixelate(2, [1, 1], 1.0);
        let quarter = 0x40_40_00_00;
        for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
            assert_eq!(buffer[y * 6 + x], quarter, "{x},{y}");
        }
        assert_eq!(buffer.iter().filter(|p| **p != 0).count(), 4);
    }

    #[test]
    fn pixelate_keeps_flat_areas_and_ignores_tiny_blocks() {
        let mut buffer = vec![RED; 25];
        let mut c = canvas(&mut buffer, 5);
        c.pixelate(3, [2, 7], 1.0);
        c.pixelate(1, [0, 0], 1.0);
        c.pixelate(0, [0, 0], 1.0);
        assert!(buffer.iter().all(|p| *p == RED));
    }

    #[test]
    fn partial_pixelate_moves_pixels_only_part_of_the_way() {
        let mut buffer = vec![0u32; 4];
        buffer[0] = RED;
        let mut c = canvas(&mut buffer, 2);
        c.pixelate(2, [0, 0], 0.5);
        c.pixelate(2, [0, 0], 0.0);
        let alpha = |p: u32| p >> 24;
        assert!(alpha(buffer[0]) > 0x40 && alpha(buffer[0]) < 0xFF);
        assert!(alpha(buffer[3]) > 0 && alpha(buffer[3]) < 0x40);
    }

    fn sprite(pixels: &[u32], side: usize) -> Sprite<'_> {
        Sprite {
            pixels,
            stride: side,
            rect: [0, 0, side as u32, side as u32],
        }
    }

    #[test]
    fn new_rejects_short_slice() {
        let mut buffer = [0u32; 15];
        assert!(Canvas::new(&mut buffer, 4, 4, 4).is_none());
        assert!(Canvas::new(&mut buffer, 3, 4, 4).is_none());
        let mut buffer = [0u32; 7];
        assert!(Canvas::new(&mut buffer, 4, 3, 2).is_some());
    }

    #[test]
    fn blit_clips_negative_and_overflowing() {
        let mut buffer = [0u32; 16];
        let mut canvas = canvas(&mut buffer, 4);
        let pixels = sheet(4, 4, RED);
        canvas.blit(sprite(&pixels, 4), Vec2::new(-2.0, -2.0), 1.0, false, 1.0);
        canvas.blit(sprite(&pixels, 4), Vec2::new(3.0, 3.0), 1.0, false, 1.0);
        canvas.blit(sprite(&pixels, 4), Vec2::new(1e30, -1e30), 1.0, false, 1.0);
        assert_eq!(canvas.alpha_at(0, 0), 255);
        assert_eq!(canvas.alpha_at(1, 1), 255);
        assert_eq!(canvas.alpha_at(2, 2), 0);
        assert_eq!(canvas.alpha_at(3, 3), 255);
        assert_eq!(canvas.alpha_at(3, 0), 0);
    }

    #[test]
    fn blit_flips_horizontally() {
        let mut buffer = [0u32; 4];
        let mut canvas = Canvas::new(&mut buffer, 2, 2, 2).unwrap();
        let pixels = [RED, 0, RED, 0];
        canvas.blit(sprite(&pixels, 2), Vec2::ZERO, 1.0, true, 1.0);
        assert_eq!([canvas.alpha_at(0, 0), canvas.alpha_at(1, 0)], [0, 255]);
    }

    #[test]
    fn blit_scales_by_two() {
        let mut buffer = [0u32; 16];
        let mut canvas = canvas(&mut buffer, 4);
        let pixels = [RED, 0, 0, RED];
        canvas.blit(sprite(&pixels, 2), Vec2::ZERO, 2.0, false, 1.0);
        let row = |y| [0, 1, 2, 3].map(|x| canvas.alpha_at(x, y));
        assert_eq!(row(0), [255, 255, 0, 0]);
        assert_eq!(row(1), [255, 255, 0, 0]);
        assert_eq!(row(2), [0, 0, 255, 255]);
        assert_eq!(row(3), [0, 0, 255, 255]);
    }

    #[test]
    fn blit_applies_opacity() {
        let mut buffer = [0u32; 1];
        let mut canvas = Canvas::new(&mut buffer, 1, 1, 1).unwrap();
        canvas.blit(sprite(&[RED], 1), Vec2::ZERO, 1.0, false, 0.5);
        assert_eq!(canvas.alpha_at(0, 0), 128);
    }

    #[test]
    fn blit_draws_nothing_for_bad_input() {
        let mut buffer = [0u32; 4];
        let mut canvas = Canvas::new(&mut buffer, 2, 2, 2).unwrap();
        let pixels = sheet(2, 2, RED);
        let good = sprite(&pixels, 2);
        for rect in [
            [1, 0, 2, 2],
            [0, 1, 2, 2],
            [0, 0, 3, 1],
            [u32::MAX, 0, 2, 2],
            [0, 0, 0, 2],
        ] {
            canvas.blit(Sprite { rect, ..good }, Vec2::ZERO, 1.0, false, 1.0);
        }
        for scale in [f32::NAN, -1.0, 0.0, f32::INFINITY] {
            canvas.blit(good, Vec2::ZERO, scale, false, 1.0);
        }
        canvas.blit(good, Vec2::new(f32::NAN, 0.0), 1.0, false, 1.0);
        canvas.blit(good, Vec2::ZERO, 1.0, false, f32::NAN);
        canvas.blit(good, Vec2::ZERO, 1e30, false, 1.0);
        assert_eq!(canvas.alpha_at(0, 0), 255);
        canvas.clear();
        assert_eq!(canvas.alpha_at(0, 0), 0);
    }

    #[test]
    fn overlapping_draws_stay_premultiplied() {
        let mut buffer = [0u32; 32 * 32];
        let mut canvas = canvas(&mut buffer, 32);
        let pixels = sheet(8, 8, 0x80_80_40_20);
        for i in 0..6 {
            let offset = i as f32 * 2.5;
            canvas.blit(
                sprite(&pixels, 8),
                Vec2::new(offset, offset),
                1.5,
                false,
                0.7,
            );
            canvas.fill_circle(Vec2::new(10.0 + offset, 12.0), 7.3, Color(0xFF_80_10), 0.6);
            canvas.fill_round_rect(
                Rect::from_xywh(offset, 4.0, 14.0, 9.0),
                4.0,
                Color(0x20_A0_FF),
                0.5,
            );
            canvas.blit_mask(
                &[200; 12],
                4,
                3,
                Vec2::new(offset, 20.0),
                Color(0xFF_FF_FF),
                0.9,
            );
        }
        for pixel in buffer {
            let alpha = pixel >> 24;
            assert!(
                [pixel >> 16 & 255, pixel >> 8 & 255, pixel & 255]
                    .iter()
                    .all(|c| *c <= alpha),
                "{pixel:08x}"
            );
        }
    }

    #[test]
    fn circle_is_antialiased_and_clipped() {
        let mut buffer = [0u32; 100];
        let mut canvas = Canvas::new(&mut buffer, 10, 10, 10).unwrap();
        canvas.fill_circle(Vec2::new(5.0, 5.0), 3.2, Color(0xFF_FF_FF), 1.0);
        assert_eq!(canvas.alpha_at(5, 5), 255);
        assert_eq!(canvas.alpha_at(0, 0), 0);
        let edge = canvas.alpha_at(8, 5);
        assert!(edge > 0 && edge < 255, "{edge}");
        canvas.fill_circle(Vec2::new(-1e9, 5.0), 3.0, Color(0xFF_FF_FF), 1.0);
        canvas.fill_circle(Vec2::new(5.0, 5.0), f32::NAN, Color(0xFF_FF_FF), 1.0);
    }

    #[test]
    fn ellipse_follows_radii_and_rotation() {
        let mut buffer = [0u32; 400];
        let mut canvas = Canvas::new(&mut buffer, 20, 20, 20).unwrap();
        let white = Color(0xFF_FF_FF);
        canvas.fill_ellipse(Vec2::new(10.0, 10.0), Vec2::new(7.6, 3.0), 0.0, white, 1.0);
        assert_eq!(canvas.alpha_at(10, 10), 255);
        assert_eq!(canvas.alpha_at(16, 10), 255);
        assert_eq!(canvas.alpha_at(10, 15), 0);
        let edge = canvas.alpha_at(17, 10);
        assert!(edge > 0 && edge < 255, "{edge}");
        canvas.clear();
        let quarter = std::f32::consts::FRAC_PI_2;
        canvas.fill_ellipse(
            Vec2::new(10.0, 10.0),
            Vec2::new(8.0, 3.0),
            quarter,
            white,
            1.0,
        );
        assert_eq!(canvas.alpha_at(10, 16), 255);
        assert_eq!(canvas.alpha_at(16, 10), 0);
    }

    #[test]
    fn ellipse_is_clipped_and_ignores_bad_input() {
        let mut buffer = [0u32; 100];
        let mut canvas = Canvas::new(&mut buffer, 10, 10, 10).unwrap();
        let white = Color(0xFF_FF_FF);
        canvas.fill_ellipse(Vec2::new(-1e9, 5.0), Vec2::new(3.0, 2.0), 0.3, white, 1.0);
        canvas.fill_ellipse(
            Vec2::new(5.0, 5.0),
            Vec2::new(f32::NAN, 2.0),
            0.0,
            white,
            1.0,
        );
        canvas.fill_ellipse(Vec2::new(5.0, 5.0), Vec2::new(3.0, 0.0), 0.0, white, 1.0);
        canvas.fill_ellipse(
            Vec2::new(5.0, 5.0),
            Vec2::new(3.0, 2.0),
            f32::NAN,
            white,
            1.0,
        );
        canvas.fill_ellipse(
            Vec2::new(5.0, 5.0),
            Vec2::new(3.0, 2.0),
            0.0,
            white,
            f32::NAN,
        );
        assert!((0..10).all(|x| (0..10).all(|y| canvas.alpha_at(x, y) == 0)));
        canvas.fill_ellipse(Vec2::new(5.0, 5.0), Vec2::new(1e9, 1e9), 0.0, white, 1.0);
        assert_eq!(canvas.alpha_at(9, 9), 255);
    }

    #[test]
    fn capsule_covers_segment_and_tapers() {
        let mut buffer = [0u32; 600];
        let mut canvas = Canvas::new(&mut buffer, 30, 30, 20).unwrap();
        let white = Color(0xFF_FF_FF);
        canvas.fill_capsule(
            Vec2::new(5.0, 10.0),
            Vec2::new(25.0, 10.0),
            4.0,
            1.0,
            white,
            1.0,
        );
        assert_eq!(canvas.alpha_at(5, 10), 255);
        assert_eq!(canvas.alpha_at(15, 10), 255);
        assert_eq!(canvas.alpha_at(2, 10), 255);
        // Толстый конец шире тонкого
        assert_eq!(canvas.alpha_at(5, 7), 255);
        assert_eq!(canvas.alpha_at(25, 7), 0);
        assert_eq!(canvas.alpha_at(15, 3), 0);
    }

    #[test]
    fn capsule_degenerate_and_bad_input() {
        let mut buffer = [0u32; 100];
        let mut canvas = Canvas::new(&mut buffer, 10, 10, 10).unwrap();
        let white = Color(0xFF_FF_FF);
        let at = Vec2::new(5.0, 5.0);
        canvas.fill_capsule(at, at, 2.0, 2.0, white, 1.0);
        assert_eq!(canvas.alpha_at(5, 5), 255);
        canvas.clear();
        canvas.fill_capsule(Vec2::new(f32::NAN, 5.0), at, 2.0, 2.0, white, 1.0);
        canvas.fill_capsule(at, at, 0.0, 0.0, white, 1.0);
        canvas.fill_capsule(at, at, -1.0, 2.0, white, 1.0);
        canvas.fill_capsule(
            Vec2::new(-1e9, -1e9),
            Vec2::new(-1e9, 1e9),
            3.0,
            3.0,
            white,
            1.0,
        );
        assert!((0..10).all(|x| (0..10).all(|y| canvas.alpha_at(x, y) == 0)));
    }

    #[test]
    fn round_rect_rounds_corners() {
        let mut buffer = [0u32; 400];
        let mut canvas = Canvas::new(&mut buffer, 20, 20, 20).unwrap();
        canvas.fill_round_rect(
            Rect::from_xywh(2.0, 2.0, 16.0, 16.0),
            6.0,
            Color(0xFF_00_00),
            1.0,
        );
        assert_eq!(canvas.alpha_at(10, 10), 255);
        assert_eq!(canvas.alpha_at(2, 2), 0);
        assert_eq!(canvas.alpha_at(10, 2), 255);
    }

    #[test]
    fn mask_tints_with_coverage() {
        let mut buffer = [0u32; 4];
        let mut canvas = Canvas::new(&mut buffer, 2, 2, 2).unwrap();
        canvas.blit_mask(
            &[255, 0, 128, 255],
            2,
            2,
            Vec2::ZERO,
            Color(0xFF_FF_FF),
            1.0,
        );
        assert_eq!(canvas.alpha_at(0, 0), 255);
        assert_eq!(canvas.alpha_at(1, 0), 0);
        assert_eq!(canvas.alpha_at(0, 1), 128);
        canvas.blit_mask(&[255], 2, 2, Vec2::ZERO, Color(0), 1.0);
        canvas.blit_mask(&[255; 4], 2, 2, Vec2::new(-1e12, 0.0), Color(0), 1.0);
        assert_eq!(canvas.alpha_at(-1, 0), 0);
    }

    #[test]
    fn clear_touches_only_canvas_region() {
        let mut buffer = [7u32; 12];
        let mut canvas = Canvas::new(&mut buffer, 4, 2, 2).unwrap();
        canvas.clear();
        assert_eq!(buffer, [0, 0, 7, 7, 0, 0, 7, 7, 7, 7, 7, 7]);
    }

    #[test]
    fn clear_outside_keeps_only_the_inner_rect() {
        let mut buffer = [7u32; 9];
        let mut canvas = Canvas::new(&mut buffer, 3, 3, 3).unwrap();
        canvas.clear_outside(Rect::new(Vec2::new(1.0, 0.0), Vec2::new(9.0, 2.0)));
        assert_eq!(buffer, [0, 7, 7, 0, 7, 7, 0, 0, 0]);
    }
}
