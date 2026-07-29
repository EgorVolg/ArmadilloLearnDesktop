use std::{
    sync::mpsc::Sender,
    thread::{self, JoinHandle},
};

use windows::Win32::{
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_CONTROL, VK_P},
        WindowsAndMessaging::{
            DispatchMessageW, GetCursorPos, GetMessageW, PostThreadMessageW, TranslateMessage, MSG,
            WM_HOTKEY, WM_QUIT,
        },
    },
};

use crate::app_core::input::event::InputEvent;

pub struct HotkeyHook {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl HotkeyHook {
    pub fn start(tx: Sender<InputEvent>) -> Result<Self, windows::core::Error> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u32>();

        let thread = thread::spawn(move || {
            unsafe {
                let _ = RegisterHotKey(None, 1, MOD_CONTROL, VK_P.0 as u32);
            }
            let thread_id = unsafe { GetCurrentThreadId() };

            ready_tx.send(thread_id).unwrap();

            let mut msg = MSG::default();

            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
                if msg.message == WM_HOTKEY {
                    unsafe {
                        let mut point = Default::default();

                        GetCursorPos(&mut point).unwrap();

                        let _ = tx.send(InputEvent::Lookup {
                            x: point.x,
                            y: point.y,
                        });
                    }
                }

                unsafe {
                    let _ = TranslateMessage(&msg);

                    DispatchMessageW(&msg);
                }
            }

            unsafe {
                let _ = UnregisterHotKey(None, 1);
            }
        });

        let thread_id = ready_rx.recv().unwrap();

        Ok(Self {
            thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for HotkeyHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_QUIT,
                Default::default(),
                Default::default(),
            );
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
