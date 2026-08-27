use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};

use tauri::AppHandle;

use crate::app_core::{
    input::{event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook},
    lookup::{
        pipeline::ClickPipeline,
        provider::{_trait::AiProvider, LocalProvider},
    },
    ocr::engine::OcrEngine,
    overlay::manager::OverlayManager,
};

/// Runtime приложения.
///
/// Хранит системные hooks и запускает основной ClickPipeline.
/// ClickPipeline живёт в отдельном потоке и повторно используется
/// для обработки всех Lookup-событий.
pub struct AppRuntime {
    _mouse: MouseHook,
    _hotkey: HotkeyHook,
}

impl AppRuntime {
    /// Создаёт runtime приложения.
    pub fn new(app: AppHandle) -> Self {
        // =================================================
        // INPUT EVENTS
        // =================================================

        let (tx, rx) = mpsc::channel::<InputEvent>();

        // =================================================
        // OVERLAY
        // =================================================

        let overlay = Arc::new(OverlayManager::new(app.clone()));

        // =================================================
        // AI PROVIDER
        // =================================================
        //
        // Сейчас используем LocalProvider.
        //
        // Другие провайдеры остаются доступными и могут
        // быть подключены позже.
        //

        let provider: Arc<dyn AiProvider> =
            Arc::new(LocalProvider::new().expect("Failed to create Local provider"));

        // =================================================
        // OCR
        // =================================================
        //
        // OCR engine создаётся ОДИН РАЗ при старте приложения.
        //
        // ONNX-модели не должны загружаться при каждом клике.
        //

        let model_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("app_core")
            .join("ocr")
            .join("ppocrv5-en");

        let ocr = OcrEngine::new(model_dir).expect("Failed to initialize OCR engine");

        let ocr = Arc::new(Mutex::new(ocr));

        // =================================================
        // PIPELINE
        // =================================================

        let mut pipeline = ClickPipeline::new(overlay.clone(), app.clone(), provider, ocr);

        // =================================================
        // MOUSE HOOK
        // =================================================

        let mouse = MouseHook::start(tx.clone()).expect("Failed to start mouse hook");

        // =================================================
        // HOTKEY HOOK
        // =================================================

        let hotkey = HotkeyHook::start(tx.clone()).expect("Failed to start hotkey hook");

        // =================================================
        // EVENT LOOP
        // =================================================

        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                pipeline.process(event);
            }
        });

        // =================================================
        // RUNTIME
        // =================================================

        Self {
            _mouse: mouse,
            _hotkey: hotkey,
        }
    }
}
