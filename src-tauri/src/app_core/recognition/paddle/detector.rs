use ort::{ session::Session, value::Tensor };

use crate::app_core::recognition::{ types::OcrError };

use super::preprocess::DetectionInput;

/// ONNX detector для PP-OCRv5.
///
/// Отвечает только за:
/// - загрузку detection-модели;
/// - подготовку входного tensor;
/// - запуск ONNX inference.
///
/// Преобразование результата модели в текстовые области
/// выполняется отдельно в `postprocess.rs`.
use std::sync::Mutex;

/// PP-OCRv5 detection engine.
///
/// Хранит ONNX Session внутри Mutex, потому что
/// ort::Session::run() в используемой версии ort
/// требует изменяемый доступ к Session.
pub struct PaddleDetector {
    /// ONNX Runtime session.
    ///
    /// Mutex позволяет выполнять inference через `&self`,
    /// не заставляя весь RecognitionPipeline быть mutable.
    session: Mutex<Session>,
}

impl PaddleDetector {
    /// Создаёт detector и загружает PP-OCRv5 detection model.
    pub fn new() -> Result<Self, OcrError> {
        println!("=== INITIALIZING PADDLE DETECTOR ===");

        let model_path = "models/det/inference.onnx";

        println!("Loading detection model: {model_path}");

        // Отключаем memory pattern, потому что detection-модель
        // использует динамические размеры входного изображения.
        //
        // Иначе ONNX Runtime может попытаться повторно использовать
        // буфер от предыдущей формы tensor.
        let session = Session::builder()
            .map_err(|error|
                OcrError::Recognition(format!("Failed to create ONNX session: {error:?}"))
            )?
            .with_memory_pattern(false)
            .map_err(|error|
                OcrError::Recognition(format!("Failed to configure ONNX session: {error:?}"))
            )?
            .commit_from_file(model_path)
            .map_err(|error|
                OcrError::Recognition(format!("Failed to load detection model: {error:?}"))
            )?;
            
        println!("Detection model loaded successfully.");
        println!("Inputs: {}", session.inputs().len());
        println!("Outputs: {}", session.outputs().len());

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Запускает detection inference для изображения.
    ///
    /// На выходе пока возвращаем сырые данные модели.
    /// DBNet postprocessing будет добавлен следующим этапом.
    pub fn detect(&self, input: DetectionInput) -> Result<(), OcrError> {
        let mut session = self.session
            .lock()
            .map_err(|error| {
                OcrError::Recognition(format!("Failed to lock ONNX session: {error}"))
            })?;
        println!("=== PADDLE DETECTION START ===");

        println!("Input tensor shape: [1, 3, {}, {}]", input.height, input.width);

        let shape = [1usize, 3usize, input.height, input.width];

        let tensor = Tensor::from_array((shape, input.data)).map_err(|error| {
            OcrError::Engine(format!("Failed to create input tensor: {error}"))
        })?;

        println!("Input tensor created.");
        println!("STARTING ONNX INFERENCE...");

        // Передаём tensor в PP-OCRv5 detection model.
        //
        // `Session::run` принимает &mut self, поэтому detector
        // должен владеть изменяемой ONNX session.
        let outputs = session
            .run(ort::inputs![tensor])
            .map_err(|error| {
                OcrError::Recognition(format!("ONNX inference failed: {error:?}"))
            })?;
        println!("ONNX inference completed.");
        println!("Outputs returned: {}", outputs.len());

        println!("=== PADDLE DETECTION END ===");

        Ok(())
    }
}
