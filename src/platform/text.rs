//! Растеризация текста через GDI в карту покрытия (серое сглаживание, не ClearType).

use windows_sys::{
    Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            ANTIALIASED_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
            DT_CALCRECT, DT_END_ELLIPSIS, DT_NOPREFIX, DT_WORDBREAK, DeleteObject, DrawTextW,
            FF_DONTCARE, FW_BOLD, FW_NORMAL, GdiFlush, HDC, HGDIOBJ, OUT_DEFAULT_PRECIS,
            SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
    },
    w,
};

use super::{last_error, surface::Surface};
use crate::error::AgentError;

pub struct TextMask {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}

/// Шрифт, выбранный в DC; в `Drop` возвращает прежний и удаляет свой.
struct Font {
    dc: HDC,
    font: HGDIOBJ,
    previous: HGDIOBJ,
}

impl Font {
    fn select(dc: HDC, pixel_height: u32, bold: bool) -> Result<Self, AgentError> {
        let weight = if bold { FW_BOLD } else { FW_NORMAL };
        // SAFETY: имя гарнитуры нуль-терминировано; созданный шрифт сразу получает владельца.
        unsafe {
            let font = CreateFontW(
                -(pixel_height.min(i32::MAX as u32) as i32),
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                ANTIALIASED_QUALITY as u32,
                (DEFAULT_PITCH | FF_DONTCARE) as u32,
                w!("Segoe UI"),
            );
            if font.is_null() {
                return Err(last_error("CreateFontW"));
            }
            let previous = SelectObject(dc, font);
            if previous.is_null() {
                let error = last_error("SelectObject");
                DeleteObject(font);
                return Err(error);
            }
            Ok(Self { dc, font, previous })
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        // SAFETY: шрифт выбран в `dc` и принадлежит нам; снимаем перед удалением.
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.font);
        }
    }
}

const FLAGS: u32 = DT_WORDBREAK | DT_NOPREFIX;

fn draw(dc: HDC, text: &[u16], rect: &mut RECT, flags: u32) -> i32 {
    // SAFETY: буфер и RECT валидны на время вызова; длина передаётся явно.
    unsafe { DrawTextW(dc, text.as_ptr(), text.len() as i32, rect, flags) }
}

fn measure(dc: HDC, text: &[u16], max_width: u32) -> RECT {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: max_width.min(i32::MAX as u32) as i32,
        bottom: 0,
    };
    draw(dc, text, &mut rect, FLAGS | DT_CALCRECT);
    rect
}

/// Самый длинный префикс с «…», который влезает целыми строками.
fn fit(dc: HDC, text: &[u16], max_width: u32, max_height: u32) -> Vec<u16> {
    let candidate = |n: usize| {
        let mut n = n;
        // Не рвём суррогатную пару
        if n > 0 && (0xD800..0xDC00).contains(&text[n - 1]) {
            n -= 1;
        }
        let head = String::from_utf16_lossy(&text[..n]);
        let mut out: Vec<u16> = head.trim_end().encode_utf16().collect();
        out.push('…' as u16);
        out
    };
    let (mut low, mut high) = (0, text.len());
    while low < high {
        let mid = (low + high).div_ceil(2);
        if measure(dc, &candidate(mid), max_width).bottom.max(0) as u32 <= max_height {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    candidate(low)
}

pub fn rasterize(
    text: &str,
    pixel_height: u32,
    max_width: u32,
    max_height: u32,
    bold: bool,
) -> Result<TextMask, AgentError> {
    let empty = TextMask {
        width: 0,
        height: 0,
        coverage: Vec::new(),
    };
    if text.is_empty() || pixel_height == 0 || max_width == 0 || max_height == 0 {
        return Ok(empty);
    }
    let mut text: Vec<u16> = text.encode_utf16().collect();
    // Поверхность нужна и как контекст для замера, и как холст для рисования.
    let mut surface = Surface::new(max_width, max_height)?;
    let dc = surface.dc();
    let _font = Font::select(dc, pixel_height, bold)?;

    let mut measured = measure(dc, &text, max_width);
    if measured.bottom.max(0) as u32 > max_height {
        text = fit(dc, &text, max_width, max_height);
        measured = measure(dc, &text, max_width);
    }
    let width = (measured.right.max(0) as u32).min(max_width);
    let height = (measured.bottom.max(0) as u32).min(max_height);
    if width == 0 || height == 0 {
        return Ok(empty);
    }

    let mut area = RECT {
        left: 0,
        top: 0,
        right: width as i32,
        bottom: height as i32,
    };
    // SAFETY: DC принадлежит поверхности, живущей до конца функции.
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00FF_FFFF);
    }
    draw(dc, &text, &mut area, FLAGS | DT_END_ELLIPSIS);
    // SAFETY: дожидаемся GDI, прежде чем читать память DIB. Ноль — не сбой текста:
    // при пустой очереди вызова GDI он тоже возможен.
    unsafe { GdiFlush() };

    let stride = surface.width() as usize;
    let pixels = surface.pixels();
    let mut coverage = Vec::with_capacity(width as usize * height as usize);
    for row in pixels.chunks(stride).take(height as usize) {
        // Белое на чёрном: любой канал равен покрытию.
        coverage.extend(row[..width as usize].iter().map(|p| (p >> 8) as u8));
    }
    Ok(TextMask {
        width,
        height,
        coverage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_text_within_limits() {
        super::super::init_thread();
        let mask = rasterize("Привет, мир", 16, 200, 60, false).unwrap();
        assert!(mask.width > 0 && mask.width <= 200 && mask.height <= 60);
        assert_eq!(mask.coverage.len(), (mask.width * mask.height) as usize);
        assert!(mask.coverage.iter().any(|c| *c > 0));
    }

    #[test]
    fn wraps_and_clamps() {
        let long = "слово ".repeat(40);
        let mask = rasterize(&long, 14, 100, 40, true).unwrap();
        assert!(mask.width <= 100 && mask.height <= 40);
    }

    #[test]
    fn empty_text_is_empty_mask() {
        let mask = rasterize("", 16, 100, 100, false).unwrap();
        assert_eq!((mask.width, mask.height, mask.coverage.len()), (0, 0, 0));
    }
}
