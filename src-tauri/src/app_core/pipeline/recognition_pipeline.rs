use crate::app_core::recognition::{
    capture::capture_screen,
    crop::crop,
    paddle::PaddleOcrEngine,
    region::Region,
    service::RecognitionService,
    types::OcrResult,
};

/// Pipeline распознавания текста.
///
/// Выполняет последовательность:
/// capture → crop → OCR
pub struct RecognitionPipeline;

impl RecognitionPipeline {
    /// Создаёт новый pipeline.
    pub fn new() -> Self {
        Self
    }

    /// Распознаёт текст в указанной области экрана.
    pub fn run(&self, region: Region) -> Result<OcrResult, String> {
        println!("=== RECOGNITION PIPELINE START ===");

        // Захватываем экран.
        let image = capture_screen().map_err(|error| format!("{error:?}"))?;

        println!("Captured: {}x{}", image.width, image.height);

        // Обрезаем нужную область.
        let cropped = crop(&image, region).map_err(|error| format!("{error:?}"))?;

        println!("Cropped: {}x{}", cropped.width, cropped.height);

        // Создаём OCR engine.
        //
        // Пока здесь заглушка.
        println!("BEFORE ENGINE");

        let engine = PaddleOcrEngine::new().map_err(|error| format!("{error:?}"))?;

        println!("AFTER ENGINE");

        let service = RecognitionService::new(engine);

        println!("BEFORE RECOGNIZE");

        let result = service.recognize(&cropped).map_err(|error| format!("{error:?}"))?;

        println!("AFTER RECOGNIZE");

        println!("OCR regions: {}", result.regions.len());

        println!("=== RECOGNITION PIPELINE END ===");

        Ok(result)
    }
}
