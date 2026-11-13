//! Цикл ожидания: таймер, событие пробуждения и сообщения окон без опроса.

use std::{ptr, sync::Arc, time::Duration};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::Threading::{
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CancelWaitableTimer, CreateEventW,
        CreateWaitableTimerExW, INFINITE, SetEvent, SetWaitableTimer, TIMER_ALL_ACCESS,
    },
    UI::WindowsAndMessaging::{
        DispatchMessageW, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, WM_QUIT,
    },
};

use super::last_error;
use crate::error::AgentError;

struct Event(HANDLE);

// SAFETY: описатель события не привязан к потоку, SetEvent потокобезопасен.
unsafe impl Send for Event {}
// SAFETY: см. выше; других операций над описателем из чужих потоков нет.
unsafe impl Sync for Event {}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: описатель принадлежит нам и закрывается один раз.
        unsafe { CloseHandle(self.0) };
    }
}

/// Будит поток персонажа из любого другого потока.
#[derive(Clone)]
pub struct Waker(Arc<Event>);

impl Waker {
    pub fn wake(&self) {
        // SAFETY: описатель жив, пока жив `Arc`.
        unsafe { SetEvent(self.0.0) };
    }
}

pub enum Wake {
    Tick,
    Woken,
    Input,
    Closed,
}

pub struct Pump {
    timer: HANDLE,
    event: Arc<Event>,
    interval: Option<Duration>,
}

impl Pump {
    pub fn new() -> Result<(Self, Waker), AgentError> {
        // SAFETY: аргументы — null или значения; созданные описатели сразу получают владельца.
        unsafe {
            let event = CreateEventW(ptr::null(), 0, 0, ptr::null());
            if event.is_null() {
                return Err(last_error("CreateEventW"));
            }
            let event = Arc::new(Event(event));
            let mut timer = CreateWaitableTimerExW(
                ptr::null(),
                ptr::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS,
            );
            if timer.is_null() {
                // Старые сборки Windows 10 не знают флага высокого разрешения.
                timer = CreateWaitableTimerExW(ptr::null(), ptr::null(), 0, TIMER_ALL_ACCESS);
            }
            if timer.is_null() {
                return Err(last_error("CreateWaitableTimerExW"));
            }
            let waker = Waker(Arc::clone(&event));
            Ok((
                Self {
                    timer,
                    event,
                    interval: None,
                },
                waker,
            ))
        }
    }

    /// `None` отменяет таймер: поток спит до пробуждения или ввода.
    pub fn set_interval(&mut self, interval: Option<Duration>) {
        if self.interval == interval {
            return;
        }
        self.interval = interval;
        self.arm();
    }

    fn arm(&self) {
        // SAFETY: таймер принадлежит нам; относительное время — отрицательные 100 нс.
        unsafe {
            match self.interval {
                Some(interval) => {
                    let due = -i64::try_from(interval.as_nanos() / 100)
                        .unwrap_or(i64::MAX)
                        .max(1);
                    SetWaitableTimer(self.timer, &due, 0, None, ptr::null(), 0);
                }
                None => {
                    CancelWaitableTimer(self.timer);
                }
            }
        }
    }

    /// Блокируется до таймера, пробуждения или ввода; очередь сообщений опустошается всегда.
    pub fn wait(&mut self) -> Wake {
        let handles = [self.timer, self.event.0];
        // SAFETY: массив живёт до конца вызова, описатели валидны.
        let result = unsafe {
            MsgWaitForMultipleObjectsEx(
                2,
                handles.as_ptr(),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        let ticked = result == WAIT_OBJECT_0;
        if ticked {
            // До разбора сообщений, чтобы их обработка не растягивала период.
            self.arm();
        }
        if drain_messages() || result == u32::MAX {
            return Wake::Closed;
        }
        match result {
            WAIT_OBJECT_0 => Wake::Tick,
            r if r == WAIT_OBJECT_0 + 1 => Wake::Woken,
            _ => Wake::Input,
        }
    }
}

/// `true`, если получен WM_QUIT.
fn drain_messages() -> bool {
    // SAFETY: MSG заполняется системой, окна принадлежат этому потоку.
    unsafe {
        let mut msg = std::mem::zeroed();
        while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_QUIT {
                return true;
            }
            DispatchMessageW(&msg);
        }
    }
    false
}

impl Drop for Pump {
    fn drop(&mut self) {
        // SAFETY: таймер принадлежит нам и закрывается один раз.
        unsafe { CloseHandle(self.timer) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn ticks_and_wakes() {
        let (mut pump, waker) = Pump::new().unwrap();
        pump.set_interval(Some(Duration::from_millis(5)));
        assert!(matches!(pump.wait(), Wake::Tick));
        assert!(matches!(pump.wait(), Wake::Tick));
        pump.set_interval(None);
        let remote = waker.clone();
        let thread = thread::spawn(move || remote.wake());
        thread.join().unwrap();
        assert!(matches!(pump.wait(), Wake::Woken));
    }
}
