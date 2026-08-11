use std::sync::Arc;

use crate::app_core::input::event::InputEvent;
use crate::app_core::overlay::manager::OverlayManager;

/// Обрабатывает пользовательские действия,
/// связанные с поиском и переводом текста.
pub struct ClickPipeline {
    /// Менеджер overlay-окна.
    overlay: Arc<OverlayManager>,
}

impl ClickPipeline {
    /// Создаёт новый ClickPipeline.
    pub fn new(overlay: Arc<OverlayManager>) -> Self {
        Self { overlay }
    }

    /// Обрабатывает входное событие.
    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                println!("ClickPipeline: Lookup at ({x}, {y})");

                // Пока pipeline только показывает overlay.
                //
                // Capture → Crop → OCR подключим сюда
                // после того, как закончим тестировать
                // RecognitionPipeline отдельно.
                self.overlay.show(x, y);
            }
        }
    }
}