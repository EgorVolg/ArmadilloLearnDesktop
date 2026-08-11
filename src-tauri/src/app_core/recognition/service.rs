use super::image::Image;
use super::ocr::OcrEngine;
use super::types::{OcrError, OcrResult};

/// Сервис высокого уровня для работы с OCR.
///
/// Он не знает, какая конкретно OCR-модель используется.
pub struct RecognitionService<E: OcrEngine> {
    engine: E,
}

impl<E: OcrEngine> RecognitionService<E> {
    /// Создаёт RecognitionService.
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// Распознаёт текст на изображении.
    pub fn recognize(&self, image: &Image) -> Result<OcrResult, OcrError> {
        self.engine.recognize(image)
    }
}
