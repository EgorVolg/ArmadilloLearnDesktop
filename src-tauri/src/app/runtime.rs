use std::{ sync::{ mpsc, Arc }, thread };

use tauri::AppHandle;

use crate::app_core::{
    input::{ event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook },
    lookup::{
        pipeline::ClickPipeline,
        provider::{ _trait::AiProvider, GeminiProvider, GroqProvider, LocalProvider },
    },
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
        // Сейчас намеренно используем Groq.
        //
        // Gemini при этом остаётся реализованным в
        // provider/gemini.rs и может быть подключён позже.
        //

        let provider: Arc<dyn AiProvider> = Arc::new(
            // GroqProvider::new().expect("Failed to create Groq provider")
            // GeminiProvider::new().expect("Failed to create Gemini provider")
            LocalProvider::new().expect("Failed to create Local provider")
        );

        // =================================================
        // PIPELINE
        // =================================================

        let mut pipeline = ClickPipeline::new(overlay.clone(), app.clone(), provider);

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
