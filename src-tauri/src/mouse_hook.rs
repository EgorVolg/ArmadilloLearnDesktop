use std::sync::Arc;
use std::{
    sync::{
        mpsc::{self, Sender},
        OnceLock,
    },
    thread::{self, JoinHandle},
};
use windows::Win32::UI::WindowsAndMessaging::PEEK_MESSAGE_REMOVE_TYPE;
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL,
        WM_MBUTTONDOWN, WM_QUIT,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct MouseClick {
    pub x: i32,
    pub y: i32,
}

pub struct MouseHook {
    stop_thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

struct HookState {
    tx: Sender<(i32, i32)>,
}

impl MouseHook {
    pub fn install(tx: Sender<(i32, i32)>) -> Result<Self, windows::core::Error> {
        //
        // Сначала регистрируем sender,
        // чтобы callback никогда не поймал None.
        //

        let state = Arc::new(HookState { tx });

        set_state(state);

        let (ready_tx, ready_rx) = mpsc::channel::<u32>();

        let thread = thread::spawn(move || {
            unsafe {
                let mut msg = MSG::default();

                let _ = PeekMessageW(&mut msg, None, 0, 0, PEEK_MESSAGE_REMOVE_TYPE(0));
            }
            let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            ready_tx.send(thread_id).unwrap();

            let hook = unsafe {
                SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook_proc),
                    Some(HINSTANCE::default()),
                    0,
                )
            }
            .expect("Failed to install mouse hook");

            let mut msg = MSG::default();

            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            unsafe {
                if let Err(e) = UnhookWindowsHookEx(hook) {
                    eprintln!("Failed to unhook mouse hook: {}", e);
                }
            }
        });

        let thread_id = ready_rx.recv().expect("Failed to get hook thread id");

        Ok(Self {
            stop_thread_id: thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for MouseHook {
    fn drop(&mut self) {
        //
        // Просим message loop завершиться
        //

        unsafe {
            let _ = PostThreadMessageW(self.stop_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ======================================================
// WinAPI callback
// ======================================================

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam.0 == WM_MBUTTONDOWN as usize {
        let click = unsafe { mouse_click_from_lparam(lparam) };

        send_click(click);
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe fn mouse_click_from_lparam(lparam: LPARAM) -> MouseClick {
    //
    // SAFETY:
    //
    // WH_MOUSE_LL гарантирует,
    // что LPARAM содержит MSLLHOOKSTRUCT
    //

    let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };

    MouseClick {
        x: info.pt.x,
        y: info.pt.y,
    }
}

// ======================================================
// Callback communication
// ======================================================

static HOOK_STATE: OnceLock<Arc<HookState>> = OnceLock::new();

fn set_state(state: Arc<HookState>) {
    let _ = HOOK_STATE.set(state);
}

fn send_click(click: MouseClick) {
    if let Some(state) = HOOK_STATE.get() {
        let _ = state.tx.send((click.x, click.y));
    }
}
