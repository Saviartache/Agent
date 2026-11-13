//! Курсор и признаки того, что пользователь занят вводом.

use windows_sys::Win32::{
    Foundation::POINT,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
        },
        WindowsAndMessaging::{GetCursorPos, SetCursorPos},
    },
};

use crate::math::Vec2;

/// `None` на защищённом рабочем столе (экран блокировки).
pub fn position() -> Option<Vec2> {
    let mut at = POINT { x: 0, y: 0 };
    // SAFETY: указатель на локальную структуру.
    (unsafe { GetCursorPos(&mut at) } != 0).then(|| Vec2::new(at.x as f32, at.y as f32))
}

pub fn set_position(to: Vec2) -> bool {
    // SAFETY: вызов принимает только значения.
    unsafe { SetCursorPos(to.x.round() as i32, to.y.round() as i32) != 0 }
}

/// Нажата кнопка мыши или Esc: персонаж отдаёт управление.
pub fn user_input_active() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_ESCAPE]
        .into_iter()
        // SAFETY: вызов принимает только значение; старший бит (знак) — «нажата сейчас».
        .any(|key| unsafe { GetAsyncKeyState(i32::from(key)) } < 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cursor() {
        assert!(position().is_some());
        let _ = user_input_active();
    }
}
