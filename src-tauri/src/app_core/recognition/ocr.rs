use super::image::Image;
use super::types::{OcrError, OcrResult};

/// Общий интерфейс OCR-движка. 
pub trait OcrEngine: Send + Sync {
    /// Распознаёт текст на переданном изображении.
    ///
    /// На выходе получаем:
    /// - найденные текстовые области;
    /// - распознанный текст;
    /// - confidence каждого результата.
    fn recognize(&self, image: &Image) -> Result<OcrResult, OcrError>;
}
