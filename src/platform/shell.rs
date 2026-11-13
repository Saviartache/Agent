//! Состояние оболочки: полноэкранные приложения и презентации.

use windows_sys::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
    SHQueryUserNotificationState,
};

/// `true`, когда рисовать поверх нельзя; при сбое запроса — `false`.
pub fn user_busy() -> bool {
    let mut state = 0;
    // SAFETY: указатель на локальную переменную.
    let result = unsafe { SHQueryUserNotificationState(&mut state) };
    result >= 0
        && matches!(
            state,
            QUNS_NOT_PRESENT | QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_does_not_fail() {
        let _ = super::user_busy();
    }
}
