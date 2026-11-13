//! Рабочие области мониторов и их DPI.

use std::{mem, ptr};

use windows_sys::Win32::{
    Foundation::{LPARAM, POINT, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromPoint,
    },
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
};

use crate::math::{Bounds, Rect, Vec2};

unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, areas: LPARAM) -> i32 {
    // SAFETY: `areas` — указатель на Vec из `bounds`, живущий на всё время перечисления.
    let areas = unsafe { &mut *(areas as *mut Vec<Rect>) };
    // SAFETY: структура инициализирована нулями с корректным `cbSize`.
    let mut info: MONITORINFO = unsafe { mem::zeroed() };
    info.cbSize = mem::size_of::<MONITORINFO>() as u32;
    // SAFETY: `info` валидна, `monitor` выдан системой.
    if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
        let work = info.rcWork;
        areas.push(Rect::from_xywh(
            work.left as f32,
            work.top as f32,
            (work.right - work.left) as f32,
            (work.bottom - work.top) as f32,
        ));
    }
    1
}

/// Рабочие области всех мониторов в координатах виртуального экрана.
pub fn bounds() -> Option<Bounds> {
    let mut areas = Vec::new();
    // SAFETY: колбэк вызывается синхронно, пока `areas` на стеке.
    let ok = unsafe {
        EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(collect),
            ptr::from_mut(&mut areas) as LPARAM,
        )
    };
    if ok == 0 {
        return None;
    }
    Bounds::new(areas)
}

/// 96, если DPI узнать не удалось.
pub fn dpi_at(point: Vec2) -> u32 {
    let at = POINT {
        x: point.x as i32,
        y: point.y as i32,
    };
    let (mut x, mut y) = (0, 0);
    // SAFETY: выходные указатели валидны.
    let result = unsafe {
        let monitor = MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y)
    };
    if result == 0 && x != 0 { x } else { 96 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_monitors_and_dpi() {
        super::super::init_thread();
        let bounds = bounds().unwrap();
        assert!(bounds.hull().width() > 0.0);
        assert!(dpi_at(bounds.hull().center()) >= 96);
    }
}
