use crate::app_core::recognition::region::TextRegion;

pub struct RecognitionContext {
    // Координата клика
    pub cursor_x: i32,
    pub cursor_y: i32,

    // Скриншот экрана
    pub screenshot: Option<Vec<u8>>,

    // Все найденные области текста
    pub regions: Vec<TextRegion>,

    // Выбранное слово
    pub selected_region: Option<TextRegion>,

    // OCR результат
    pub word: Option<String>,

    // Исправленный результат
    pub corrected_word: Option<String>,

    // Перевод
    pub translation: Option<String>,
}

impl RecognitionContext {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            cursor_x: x,
            cursor_y: y,

            screenshot: None,

            regions: Vec::new(),

            selected_region: None,

            word: None,

            corrected_word: None,

            translation: None,
        }
    }
}
