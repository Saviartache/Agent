//! Окна, курсор, мониторы и цикл ожидания Windows. Всё живёт в одном потоке персонажа.

use windows_sys::Win32::{
    Foundation::GetLastError,
    UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
};

use crate::error::AgentError;

pub mod cursor;
pub mod monitors;
pub mod pump;
pub mod shell;
pub mod surface;
pub mod text;
pub mod window;

/// Вызывается первым в потоке персонажа; осведомлённость о DPI задаётся потоку, не процессу.
pub fn init_thread() {
    // SAFETY: вызов не принимает указателей и влияет только на текущий поток.
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

fn last_error(call: &'static str) -> AgentError {
    // SAFETY: читает код ошибки текущего потока.
    AgentError::Platform {
        call,
        code: unsafe { GetLastError() },
    }
}
