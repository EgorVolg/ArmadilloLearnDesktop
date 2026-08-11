use std::{ sync::{ mpsc, Arc }, thread };

use tauri::AppHandle;

use crate::app_core::{
    input::{ event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook },
    main_window::manager::MainWindow,
    overlay::manager::OverlayManager,
    pipeline::{ click_pipeline::ClickPipeline, recognition_pipeline::RecognitionPipeline },
};

/// Runtime приложения.
///
/// Хранит системные hooks и основной ClickPipeline.
/// RecognitionPipeline принадлежит ClickPipeline и
/// создаётся только один раз при запуске приложения.
pub struct AppRuntime {
    /// Hook мыши.
    mouse: MouseHook,

    /// Hook глобальных горячих клавиш.
    hotkey: HotkeyHook,

    /// Менеджер overlay-окна.
    overlay: Arc<OverlayManager>,

    /// Главное окно приложения.
    main_window: MainWindow,

     
}

impl AppRuntime {
    /// Создаёт runtime приложения.
    pub fn new(app: AppHandle) -> Self {
        // Создаём канал событий мыши и горячих клавиш.
        let (tx, rx) = mpsc::channel::<InputEvent>();

        // Создаём менеджер overlay.
        let overlay = Arc::new(OverlayManager::new(app.clone()));

        // Создаём главное окно.
        let main_window = MainWindow::new(app.clone());

        // Создаём RecognitionPipeline один раз.
        //
        // Здесь загружается ONNX detection model.
        let recognition = RecognitionPipeline::new().expect(
            "Failed to initialize RecognitionPipeline"
        );

        // ClickPipeline становится владельцем
        // RecognitionPipeline.
        let pipeline = ClickPipeline::new(overlay.clone(), recognition);

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
            mouse,
            hotkey,
            overlay,
            main_window,
        }
    }
}
