use std::{ sync::{ mpsc, Arc }, thread };

use tauri::AppHandle;

use crate::app_core::{
    input::{ event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook },
    overlay::manager::OverlayManager,
    pipeline::click_pipeline::ClickPipeline,
};

/// Runtime приложения.
///
/// Хранит системные hooks и основной ClickPipeline.
/// RecognitionPipeline принадлежит ClickPipeline и
/// создаётся только один раз при запуске приложения.
pub struct AppRuntime { 
    _mouse: MouseHook, 
    _hotkey: HotkeyHook,
}

impl AppRuntime {
    /// Создаёт runtime приложения.
    pub fn new(app: AppHandle) -> Self {
        // Создаём канал событий мыши и горячих клавиш.
        let (tx, rx) = mpsc::channel::<InputEvent>();

        // Создаём менеджер overlay.
        let overlay = Arc::new(OverlayManager::new(app.clone()));

        // ClickPipeline становится владельцем
        // RecognitionPipeline.
        let pipeline = ClickPipeline::new(overlay.clone(), app.clone()).expect("Click pipeline");

        // Запускаем hook мыши.
        let mouse = MouseHook::start(tx.clone()).expect("Mouse hook");

        // Запускаем hook горячих клавиш.
        let hotkey = HotkeyHook::start(tx.clone()).expect("Hotkey hook");

        // Запускаем отдельный поток обработки событий.
        //
        // Pipeline живёт внутри этого потока и
        // используется повторно для каждого клика.
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                pipeline.process(event);
            }
        });

        Self {
            _mouse: mouse,
            _hotkey: hotkey,
        }
    }
}
