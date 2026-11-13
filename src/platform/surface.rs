//! Память под пиксели окна: DIB-секция 32 бита сверху вниз в контексте памяти.

use std::{mem, ptr, slice};

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
};

use super::last_error;
use crate::error::AgentError;

const MAX_SIDE: u32 = 8192;
const GRANULARITY: u32 = 64;

pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: u32,
    height: u32,
}

fn capacity(side: u32) -> u32 {
    side.max(1).div_ceil(GRANULARITY) * GRANULARITY
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Result<Self, AgentError> {
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(AgentError::Platform {
                call: "Surface::new",
                code: 87, // ERROR_INVALID_PARAMETER
            });
        }
        let (width, height) = (capacity(width), capacity(height));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                // SAFETY: остальные поля заголовка принимают ноль.
                ..unsafe { mem::zeroed() }
            },
            // SAFETY: палитра не используется при 32 битах.
            ..unsafe { mem::zeroed() }
        };
        // SAFETY: все указатели валидны на время вызовов; каждый созданный объект
        // освобождается на ветках ошибок, иначе владение уходит в `Surface`.
        unsafe {
            let dc = CreateCompatibleDC(ptr::null_mut());
            if dc.is_null() {
                return Err(last_error("CreateCompatibleDC"));
            }
            let mut bits = ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                let error = last_error("CreateDIBSection");
                DeleteDC(dc);
                return Err(error);
            }
            let previous = SelectObject(dc, bitmap);
            if previous.is_null() {
                let error = last_error("SelectObject");
                DeleteObject(bitmap);
                DeleteDC(dc);
                return Err(error);
            }
            Ok(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    /// Только растёт: уменьшение стоило бы пересоздания ради нескольких килобайт.
    pub fn ensure(&mut self, width: u32, height: u32) -> Result<(), AgentError> {
        if width <= self.width && height <= self.height {
            return Ok(());
        }
        // Старая поверхность освобождается присваиванием.
        *self = Self::new(width.max(self.width), height.max(self.height))?;
        Ok(())
    }

    /// Ёмкость, не запрошенный размер.
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&mut self) -> &mut [u32] {
        // SAFETY: `bits` указывает на width*height u32, выделенных системой и живых,
        // пока жива `bitmap`; `&mut self` не даёт пересоздать или удалить её, пока срез используется.
        unsafe { slice::from_raw_parts_mut(self.bits, self.width as usize * self.height as usize) }
    }

    pub(super) fn dc(&self) -> HDC {
        self.dc
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: хэндлы принадлежат нам; битмап снимается с DC перед удалением.
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_by_granularity_and_never_shrinks() {
        let mut surface = Surface::new(10, 10).unwrap();
        assert_eq!((surface.width(), surface.height()), (64, 64));
        surface.ensure(65, 5).unwrap();
        assert_eq!((surface.width(), surface.height()), (128, 64));
        surface.ensure(1, 1).unwrap();
        assert_eq!((surface.width(), surface.height()), (128, 64));
        assert_eq!(surface.pixels().len(), 128 * 64);
        surface.pixels()[0] = 1;
    }

    #[test]
    fn rejects_oversize() {
        assert!(Surface::new(8193, 10).is_err());
        let mut surface = Surface::new(1, 1).unwrap();
        assert!(surface.ensure(10, 9000).is_err());
        assert_eq!(surface.width(), 64);
    }
}
