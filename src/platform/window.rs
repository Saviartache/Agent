//! Layered-окна: попиксельная альфа, без активации; события уходят через очередь потока.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    mem, ptr,
};

use windows_sys::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM},
        Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
            WindowsAndMessaging::*,
        },
    },
    w,
};

use super::{last_error, surface::Surface};
use crate::{error::AgentError, math::Vec2, render::Canvas};

const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
const QUEUE_CAP: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(isize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Interactive,
    ClickThrough,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowEvent {
    PointerDown { layer: LayerId, at: Vec2 },
    PointerUp { layer: LayerId, at: Vec2 },
    PointerCancel { layer: LayerId },
    ContextMenu { layer: LayerId, at: Vec2 },
    DisplayChanged,
    CloseRequested,
}

thread_local! {
    static EVENTS: RefCell<VecDeque<WindowEvent>> = const { RefCell::new(VecDeque::new()) };
    static PRESSED: Cell<Option<LayerId>> = const { Cell::new(None) };
}

fn push(event: WindowEvent) {
    // try_borrow: wndproc может прийти вложенно, паниковать в нём нельзя.
    let _ = EVENTS.try_with(|queue| {
        let Ok(mut queue) = queue.try_borrow_mut() else {
            return;
        };
        if event == WindowEvent::DisplayChanged && queue.back() == Some(&event) {
            return;
        }
        if queue.len() == QUEUE_CAP {
            queue.pop_front();
        }
        queue.push_back(event);
    });
}

pub fn drain_events(out: &mut Vec<WindowEvent>) {
    let _ = EVENTS.try_with(|queue| {
        if let Ok(mut queue) = queue.try_borrow_mut() {
            out.extend(queue.drain(..));
        }
    });
}

/// Экранная позиция курсора на момент сообщения.
fn message_position() -> Vec2 {
    // SAFETY: вызов без аргументов, читает состояние текущего потока.
    let packed = unsafe { GetMessagePos() };
    // Слова — знаковые 16 бит, т.к. координаты бывают отрицательными.
    Vec2::new(
        f32::from(packed as u16 as i16),
        f32::from((packed >> 16) as u16 as i16),
    )
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let layer = LayerId(hwnd as isize);
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN => {
            PRESSED.set(Some(layer));
            push(WindowEvent::PointerDown {
                layer,
                at: message_position(),
            });
            // SAFETY: окно принадлежит этому потоку.
            unsafe { SetCapture(hwnd) };
            0
        }
        WM_LBUTTONUP => {
            if PRESSED.take() == Some(layer) {
                push(WindowEvent::PointerUp {
                    layer,
                    at: message_position(),
                });
                // SAFETY: вызов без указателей; нажатие уже снято, отмены не будет.
                unsafe { ReleaseCapture() };
            }
            0
        }
        WM_CAPTURECHANGED => {
            if PRESSED.get() == Some(layer) {
                PRESSED.set(None);
                push(WindowEvent::PointerCancel { layer });
            }
            0
        }
        WM_RBUTTONUP => {
            push(WindowEvent::ContextMenu {
                layer,
                at: message_position(),
            });
            0
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            push(WindowEvent::DisplayChanged);
            0
        }
        WM_SETTINGCHANGE if wp == SPI_SETWORKAREA as WPARAM => {
            push(WindowEvent::DisplayChanged);
            0
        }
        WM_CLOSE => {
            push(WindowEvent::CloseRequested);
            0
        }
        // SAFETY: аргументы переданы системой без изменений.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn register_class() -> Result<(), AgentError> {
    // SAFETY: структура и имя класса живут до конца вызова; класс остаётся
    // зарегистрированным на весь процесс, поток можно перезапустить.
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: GetModuleHandleW(ptr::null()),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_HAND),
            lpszClassName: w!("ui_agent_layer"),
            ..mem::zeroed()
        };
        if RegisterClassExW(&class) == 0 {
            let error = last_error("RegisterClassExW");
            return match error {
                AgentError::Platform { code, .. } if code == ERROR_CLASS_ALREADY_EXISTS => Ok(()),
                _ => Err(error),
            };
        }
    }
    Ok(())
}

pub struct Layer {
    hwnd: HWND,
    surface: Surface,
    visible: bool,
}

impl Layer {
    pub fn new(kind: LayerKind) -> Result<Self, AgentError> {
        register_class()?;
        let mut style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if kind == LayerKind::ClickThrough {
            style |= WS_EX_TRANSPARENT;
        }
        let surface = Surface::new(1, 1)?;
        // SAFETY: класс зарегистрирован выше, строки нуль-терминированы.
        let hwnd = unsafe {
            CreateWindowExW(
                style,
                w!("ui_agent_layer"),
                ptr::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(last_error("CreateWindowExW"));
        }
        Ok(Self {
            hwnd,
            surface,
            visible: false,
        })
    }

    pub fn id(&self) -> LayerId {
        LayerId(self.hwnd as isize)
    }

    /// Очищенный холст ровно `width`×`height`.
    pub fn canvas(&mut self, width: u32, height: u32) -> Result<Canvas<'_>, AgentError> {
        self.surface.ensure(width, height)?;
        let stride = self.surface.width() as usize;
        Canvas::new(self.surface.pixels(), stride, width, height)
            .map(|mut canvas| {
                canvas.clear();
                canvas
            })
            .ok_or(AgentError::Platform {
                call: "Layer::canvas",
                code: 87, // ERROR_INVALID_PARAMETER
            })
    }

    /// Позиция, размер и пиксели меняются одним вызовом: без мерцания.
    pub fn present(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        opacity: u8,
    ) -> Result<(), AgentError> {
        let size = SIZE {
            cx: width.min(self.surface.width()) as i32,
            cy: height.min(self.surface.height()) as i32,
        };
        let at = POINT { x, y };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: opacity,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: окно и DC принадлежат этому потоку, структуры живут до конца вызова.
        let ok = unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                ptr::null_mut(),
                &at,
                &size,
                self.surface.dc(),
                &origin,
                0,
                &blend,
                ULW_ALPHA,
            )
        };
        if ok == 0 {
            return Err(last_error("UpdateLayeredWindow"));
        }
        Ok(())
    }

    pub fn show(&mut self) {
        // SAFETY: окно принадлежит этому потоку. Повторное закрепление поверх остальных
        // нужно: другие topmost-окна могли перекрыть наше.
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
        self.visible = true;
    }

    pub fn hide(&mut self) {
        // SAFETY: окно принадлежит этому потоку.
        unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Порядок среди своих окон, без активации.
    pub fn stack_below(&self, above: &Layer) {
        // SAFETY: оба окна принадлежат этому потоку.
        unsafe {
            SetWindowPos(
                self.hwnd,
                above.hwnd,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    /// `None`, если меню закрыто без выбора. Идентификаторы пунктов — ненулевые.
    pub fn popup_menu(&self, at: Vec2, items: &[(u32, &str)]) -> Option<u32> {
        // SAFETY: меню создаётся и уничтожается здесь же; строки нуль-терминированы
        // и живут до конца вызова AppendMenuW (меню копирует их).
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return None;
            }
            for (id, label) in items {
                let wide: Vec<u16> = label.encode_utf16().chain([0]).collect();
                AppendMenuW(menu, MF_STRING, *id as usize, wide.as_ptr());
            }
            // Без этого меню не закрывается кликом мимо.
            SetForegroundWindow(self.hwnd);
            let chosen = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                at.x as i32,
                at.y as i32,
                0,
                self.hwnd,
                ptr::null(),
            );
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            DestroyMenu(menu);
            u32::try_from(chosen).ok().filter(|id| *id != 0)
        }
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        // SAFETY: окно создано этим потоком и не уничтожалось ранее.
        unsafe { DestroyWindow(self.hwnd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Color;

    #[test]
    fn layer_lifecycle() {
        super::super::init_thread();
        let mut layer = Layer::new(LayerKind::Interactive).unwrap();
        let mut canvas = layer.canvas(100, 80).unwrap();
        canvas.fill_circle(Vec2::new(50.0, 40.0), 30.0, Color(0xFF_80_00), 1.0);
        assert_eq!(canvas.alpha_at(50, 40), 255);
        layer.present(10, 10, 100, 80, 255).unwrap();
        layer.show();
        assert!(layer.is_visible());
        layer.show();
        layer.hide();
        layer.hide();
        assert!(!layer.is_visible());
        let mut other = Layer::new(LayerKind::ClickThrough).unwrap();
        other.canvas(8, 8).unwrap();
        other.present(0, 0, 8, 8, 128).unwrap();
        other.stack_below(&layer);
        assert_ne!(layer.id(), other.id());
        let mut events = Vec::new();
        drain_events(&mut events);
    }

    #[test]
    fn queue_drops_oldest() {
        let layer = LayerId(1);
        for _ in 0..QUEUE_CAP + 10 {
            push(WindowEvent::PointerCancel { layer });
        }
        push(WindowEvent::CloseRequested);
        let mut events = Vec::new();
        drain_events(&mut events);
        assert_eq!(events.len(), QUEUE_CAP);
        assert_eq!(events.last(), Some(&WindowEvent::CloseRequested));
    }
}
