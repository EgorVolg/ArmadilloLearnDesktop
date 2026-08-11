use super::region::Region;

/// Результат полного OCR изображения.
///
/// Может содержать несколько найденных текстовых областей.
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// Все найденные области текста.
    pub regions: Vec<TextRegion>,
}

/// Одна область текста на изображении.
#[derive(Debug, Clone)]
pub struct TextRegion {
    /// Распознанный текст.
    pub text: String,

    /// Уверенность OCR-модели в результате.
    ///
    /// Обычно значение находится в диапазоне 0.0..=1.0.
    pub confidence: f32,

    /// Координаты текста внутри изображения,
    /// которое передали в OCR.
    pub region: Region,
}

/// Ошибки OCR.
#[derive(Debug)]
pub enum OcrError {
    /// Ошибка загрузки или работы OCR-модели.
    Engine(String),

    /// Некорректное изображение.
    InvalidImage(String),

    /// Ошибка выполнения OCR.
    Recognition(String),
}
