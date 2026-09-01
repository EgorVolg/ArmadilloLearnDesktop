use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use tauri::AppHandle;

use crate::app_core::{
    input::{event::InputEvent, hotkey::HotkeyHook, mouse::MouseHook}, lookup::{ 
        pipeline::ClickPipeline, provider::{_trait::AiProvider, LocalProvider},
    }, ocr::engine::OcrEngine, overlay::manager::OverlayManager,
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
        // AI WARM-UP
        // =================================================
        //
        // Загружаем модель Ollama в VRAM сразу при старте приложения
        // в фоновом потоке. Иначе первый клик ждал бы загрузку модели
        // (десятки секунд при холодном старте Ollama).
        //
        // Ошибка прогрева (например, Ollama ещё не запущена) не критична:
        // модель загрузится при первом реальном запросе.

        thread::spawn(|| {
            // Ollama может стартовать позже приложения (после ребута
            // системы), поэтому прогрев ретраится ~60 секунд, пока модель
            // не окажется в VRAM. keep_alive=-1 в прогреве и в lookup
            // удерживает её резидентной — после этого cold start невозможен.
            for _ in 0..30 {
                if LocalProvider::new()
                    .map(|provider| provider.warm_up())
                    .unwrap_or(false)
                {
                    return;
                }

                thread::sleep(Duration::from_secs(2));
            }

            println!("AI model warm-up gave up after ~60 s; the first lookup will load the model");
        });

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
        // OCR WARM-UP
        // =================================================
        //
        // Первый инференс аллоцирует memory arena и инициализирует
        // DirectML-ресурсы. Прогреваем в фоне сразу после старта,
        // чтобы первый клик не платил за это.

        {
            let ocr = Arc::clone(&ocr);

            thread::spawn(move || {
                if let Ok(mut engine) = ocr.lock() {
                    engine.warm_up();
                }
            });
        }

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
