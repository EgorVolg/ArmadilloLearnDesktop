use super::image::Image;
use super::ocr::OcrEngine;
use super::types::{ OcrError, OcrResult };

/// OCR-движок на основе PP-OCRv5.
///
/// В дальнейшем здесь будут находиться:
/// - detection model;
/// - recognition model;
/// - ONNX Runtime;
/// - preprocessing/postprocessing.
pub struct PaddleOcrEngine {
    // Пока пусто.
    //
    // Здесь позже появятся ONNX-сессии моделей.
}

impl PaddleOcrEngine {
    /// Создаёт новый PP-OCRv5 engine.
    pub fn new() -> Result<Self, OcrError> {
        Ok(Self {})
    }
}

impl OcrEngine for PaddleOcrEngine {
    /// Распознаёт текст на изображении.
    fn recognize(&self, _image: &Image) -> Result<OcrResult, OcrError> {
        println!("PaddleOcrEngine::recognize()");

        // PP-OCRv5 пока ещё не подключён.
        //
        // Возвращаем пустой результат, чтобы проверить
        // полный pipeline без настоящей модели.
        Ok(OcrResult {
            regions: Vec::new(),
        })
    }
}
