use std::{
    sync::{mpsc::Sender, Arc, OnceLock},
    thread::{self, JoinHandle},
};

use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, MSG, MSLLHOOKSTRUCT,
        PEEK_MESSAGE_REMOVE_TYPE, WH_MOUSE_LL, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MBUTTONDBLCLK,
        WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_XBUTTONDBLCLK,
        WM_XBUTTONDOWN,
    },
};

use super::event::InputEvent;

pub struct MouseHook {
    stop_thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

struct HookState {
    tx: Sender<InputEvent>,
}

static HOOK_STATE: OnceLock<Arc<HookState>> = OnceLock::new();

impl MouseHook {
    pub fn start(tx: Sender<InputEvent>) -> Result<Self, windows::core::Error> {
        let state = Arc::new(HookState { tx });

        let _ = HOOK_STATE.set(state);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u32>();

        let thread = thread::spawn(move || {
            unsafe {
                //
                // Создаем message queue
                //
                let mut msg = MSG::default();

                let _ = PeekMessageW(&mut msg, None, 0, 0, PEEK_MESSAGE_REMOVE_TYPE(0));
            }

            let thread_id = unsafe { GetCurrentThreadId() };

            ready_tx.send(thread_id).unwrap();

            //
            // Устанавливаем глобальный hook
            //
            let hook = unsafe {
                SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook_proc),
                    Some(HINSTANCE::default()),
                    0,
                )
            }
            .expect("Failed to install mouse hook");

            //
            // Message loop
            //
            let mut msg = MSG::default();

            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        });

        let thread_id = ready_rx.recv().expect("Failed to receive hook thread id");

        Ok(Self {
            stop_thread_id: thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for MouseHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.stop_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        match wparam.0 as u32 {
            // Средняя кнопка мыши (клик колесом) — lookup, как раньше.
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };

                emit(InputEvent::Lookup {
                    x: info.pt.x,
                    y: info.pt.y,
                });
            }

            // Любая другая кнопка мыши — скрываем оверлей, если клик не по
            // самому оверлею (иначе перетаскивание/клик по его кнопкам
            // было бы невозможно — оверлей прятался бы на mouse-down).
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK
            | WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };

                emit(InputEvent::DismissClick {
                    x: info.pt.x,
                    y: info.pt.y,
                });
            }

            _ => {}
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn emit(event: InputEvent) {
    if let Some(state) = HOOK_STATE.get() {
        let _ = state.tx.send(event);
    }
}
