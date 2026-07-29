use std::{sync::mpsc, thread};

use tauri::AppHandle;

use crate::app_core::{
    input::{event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook},
    overlay::manager::OverlayManager,
    pipeline::click_pipeline::ClickPipeline,
};

pub struct AppRuntime {
    mouse: MouseHook,
    hotkey: HotkeyHook,
}

impl AppRuntime {
    pub fn new(app: AppHandle) -> Self {
        //
        // Канал событий мыши
        //
        let (tx, rx) = mpsc::channel::<InputEvent>();

        //
        // Создаем Overlay
        //
        let overlay = OverlayManager::new(app);

        //
        // Создаем Pipeline
        //
        let pipeline = ClickPipeline::new(overlay);

        //
        // Запускаем Mouse Hook
        //

        let mouse = MouseHook::start(tx.clone()).expect("Mouse hook");

        let hotkey = HotkeyHook::start(tx.clone()).expect("Hotkey hook");
        //
        // Поток обработки событий
        //
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                pipeline.process(event);
            }
        });

        Self { mouse, hotkey }
    }
}
