use std::sync::Arc;

use crate::app_core::input::event::InputEvent;
use crate::app_core::overlay::manager::OverlayManager;
use crate::app_core::pipeline::recognition_pipeline::RecognitionPipeline;

/// Обрабатывает пользовательские действия,
/// связанные с поиском и переводом текста.
///
/// ClickPipeline является точкой оркестрации lookup-операции.
///
/// Сейчас цепочка выглядит так:
///
/// Click
///   ↓
/// ClickPipeline
///   ↓
/// RecognitionPipeline
///   ↓
/// Capture → Crop → Preprocess → Detection
pub struct ClickPipeline {
    /// Менеджер overlay-окна.
    overlay: Arc<OverlayManager>,

    /// Pipeline распознавания текста.
    recognition: RecognitionPipeline,
}

impl ClickPipeline {
    /// Создаёт новый ClickPipeline.
    pub fn new(overlay: Arc<OverlayManager>, recognition: RecognitionPipeline) -> Self {
        Self {
            overlay,
            recognition,
        }
    }

    /// Обрабатывает входное событие.
    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                println!("ClickPipeline: Lookup at ({x}, {y})");

                // Передаём координаты клика в RecognitionPipeline.
                //
                // Сам RecognitionPipeline после capture_screen()
                // знает реальные размеры экрана и сможет построить
                // безопасную область вокруг точки клика.
                match self.recognition.run(x, y) {
                    Ok(result) => {
                        println!("Recognition completed. Regions: {}", result.regions.len());
                    }

                    Err(error) => {
                        eprintln!("Recognition failed: {error}");
                    }
                }

                // Пока после recognition показываем overlay.
                //
                // Позже сюда можно будет передать результат
                // распознавания и координаты найденного текста.
                self.overlay.show(x, y);
            }
        }
    }
}
