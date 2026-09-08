use std::{
    sync::{mpsc::Sender, Arc, OnceLock},
    thread::{self, JoinHandle},
};

use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_P},
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, PeekMessageW,
            PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
            PEEK_MESSAGE_REMOVE_TYPE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN, MSG,
        },
    },
};

use super::event::InputEvent;

pub struct KeyboardHook {
    stop_thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

struct HookState {
    tx: Sender<InputEvent>,
}

static HOOK_STATE: OnceLock<Arc<HookState>> = OnceLock::new();

impl KeyboardHook {
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
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    Some(HINSTANCE::default()),
                    0,
                )
            }
            .expect("Failed to install keyboard hook");

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

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.stop_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        match wparam.0 as u32 {
            // Нажатие клавиши (включая Alt-комбинации). Отпускание и
            // автоповтор игнорируем: скрыть окно достаточно один раз.
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

                // Ctrl+P — глобальная комбинация lookup. Её обрабатывает
                // RegisterHotKey в hotkey.rs, здесь пропускаем, чтобы не
                // слать лишний Dismiss вместе с Lookup.
                let is_ctrl_p = info.vkCode == VK_P.0 as u32 && ctrl_pressed();

                if !is_ctrl_p {
                    // Любая другая клавиша — скрываем оверлей.
                    emit(InputEvent::Dismiss);
                }
            }

            _ => {}
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Зажата ли сейчас клавиша Control (левая или правая).
fn ctrl_pressed() -> bool {
    unsafe {
        let state = GetAsyncKeyState(VK_CONTROL.0 as i32);
        u16::from_le_bytes(state.to_le_bytes()) & 0x8000 != 0
    }
}

fn emit(event: InputEvent) {
    if let Some(state) = HOOK_STATE.get() {
        let _ = state.tx.send(event);
    }
}