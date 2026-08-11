use crate::app_core::recognition::{
    capture::capture_screen,
    crop::crop,
    paddle::{ preprocess::preprocess, PaddleDetector },
    region::Region,
    types::OcrResult,
};

/// Pipeline распознавания текста.
///
/// Отвечает за последовательность:
///
/// Capture
///   ↓
/// Crop
///   ↓
/// Preprocess
///   ↓
/// Paddle Detection
///
/// Detector создаётся один раз при создании pipeline,
/// поэтому ONNX-модель не загружается повторно при каждом клике.
pub struct RecognitionPipeline {
    /// PP-OCRv5 detection engine.
    detector: PaddleDetector,
}

impl RecognitionPipeline {
    /// Создаёт RecognitionPipeline.
    ///
    /// Здесь один раз загружается detection ONNX-модель.
    pub fn new() -> Result<Self, String> {
        println!("=== INITIALIZING RECOGNITION PIPELINE ===");

        // Загружаем Paddle detector.
        let detector = PaddleDetector::new().map_err(|error| format!("{error:?}"))?;

        println!("=== RECOGNITION PIPELINE READY ===");

        Ok(Self { detector })
    }

    /// Распознаёт текст в указанной области экрана.
    ///
    /// Пока выполняется только detection.
    pub fn run(&self, click_x: i32, click_y: i32) -> Result<OcrResult, String> {
        println!("=== RECOGNITION PIPELINE START ===");

        // Захватываем весь экран.
        let image = capture_screen().map_err(|error| format!("{error:?}"))?;

        println!("Captured: {}x{}", image.width, image.height);

        // Реальные размеры захваченного изображения.
        let screen_width = image.width as i32;
        let screen_height = image.height as i32;

        // Размер области вокруг точки клика.
        let region_width: u32 = 800;
        let region_height: u32 = 400;

        // Максимальные координаты левого верхнего угла,
        // при которых область всё ещё помещается в изображение.
        let max_x = (screen_width - (region_width as i32)).max(0);
        let max_y = (screen_height - (region_height as i32)).max(0);

        // Рассчитываем левый верхний угол области.
        let region_x = (click_x - (region_width as i32) / 2).clamp(0, max_x);

        let region_y = (click_y - (region_height as i32) / 2).clamp(0, max_y);

        let region = Region::new(region_x, region_y, region_width, region_height);

        println!(
            "Recognition region: x={}, y={}, width={}, height={}",
            region.x,
            region.y,
            region.width,
            region.height
        );

        // Обрезаем область.
        let cropped = crop(&image, region).map_err(|error| format!("{error:?}"))?;

        println!("Cropped: {}x{}", cropped.width, cropped.height);

        // Преобразуем RGB Image в формат,
        // который ожидает detection-модель.
        let input = preprocess(&cropped);

        println!("Input tensor created.");

        // Запускаем PP-OCRv5 detection.
        //
        // Detector уже загружен внутри RecognitionPipeline,
        // поэтому ONNX-модель повторно не загружается.
        self.detector.detect(input).map_err(|error| format!("{error:?}"))?;

        println!("Detection inference completed.");

        println!("=== RECOGNITION PIPELINE END ===");

        Ok(OcrResult {
            regions: Vec::new(),
        })
    }
}
