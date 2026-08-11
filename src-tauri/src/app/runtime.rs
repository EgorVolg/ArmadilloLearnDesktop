use std::{
    sync::{mpsc, Arc},
    thread,
};

use tauri::AppHandle;

use crate::app_core::{
    input::{event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook},
    main_window::manager::MainWindow,
    overlay::manager::OverlayManager,
    pipeline::click_pipeline::ClickPipeline,
};

pub struct AppRuntime {
    mouse: MouseHook,
    hotkey: HotkeyHook,

    overlay: Arc<OverlayManager>,
    main_window: MainWindow,
}

impl AppRuntime {
    pub fn new(app: AppHandle) -> Self {
        // Канал событий мыши

        let (tx, rx) = mpsc::channel::<InputEvent>();

        // Создаем Overlay
        let overlay = Arc::new(OverlayManager::new(app.clone()));
        // Создаем окно
        let main_window = MainWindow::new(app.clone());

        // Создаем Pipeline
        let pipeline = { ClickPipeline::new(overlay.clone()) };

        // Запускаем Mouse Hook
        let mouse = MouseHook::start(tx.clone()).expect("Mouse hook");

        // Запускаем Hotkey Hook
        let hotkey = HotkeyHook::start(tx.clone()).expect("Hotkey hook");

        // Поток обработки событий
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                pipeline.process(event);
            }
        });

        Self {
            mouse,
            hotkey,
            overlay,
            main_window,
        }
    }
}
