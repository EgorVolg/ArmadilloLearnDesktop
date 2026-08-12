use std::sync::Mutex;

use ort::{ session::Session, value::Tensor };

use crate::app_core::recognition::{
    paddle::postprocess::postprocess,
    region::Region,
    types::OcrError,
};

use super::preprocess::DetectionInput;

/// ONNX detector для PP-OCRv5.
///
/// Отвечает только за:
/// - загрузку detection-модели;
/// - подготовку input tensor;
/// - запуск ONNX inference;
/// - передачу probability map в postprocess.
///
/// Результат detection — Vec<Region>.
pub struct PaddleDetector {
    /// ONNX Runtime session.
    ///
    /// Mutex нужен потому, что используемая версия
    /// ort требует mutable доступ к Session::run().
    session: Mutex<Session>,
}

impl PaddleDetector {
    /// Создаёт detector и загружает PP-OCRv5 detection model.
    pub fn new() -> Result<Self, OcrError> {
        println!("=== INITIALIZING PADDLE DETECTOR ===");

        let model_path = "models/det/inference.onnx";

        println!("Loading detection model: {model_path}");

        let session = Session::builder()
            .map_err(|error| {
                OcrError::Recognition(format!("Failed to create ONNX session: {error:?}"))
            })?
            .with_memory_pattern(false)
            .map_err(|error| {
                OcrError::Recognition(format!("Failed to configure ONNX session: {error:?}"))
            })?
            .commit_from_file(model_path)
            .map_err(|error| {
                OcrError::Recognition(format!("Failed to load detection model: {error:?}"))
            })?;

        println!("Detection model loaded successfully.");
        println!("Inputs: {}", session.inputs().len());
        println!("Outputs: {}", session.outputs().len());

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Запускает detection inference.
    ///
    /// Возвращает bounding boxes после postprocess.
    pub fn detect(&self, input: DetectionInput) -> Result<Vec<Region>, OcrError> {
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

        let outputs = session
            .run(ort::inputs![tensor])
            .map_err(|error| {
                OcrError::Recognition(format!("ONNX inference failed: {error:?}"))
            })?;

        println!("ONNX inference completed.");
        println!("Outputs returned: {}", outputs.len());

        // Получаем output по имени модели.
        let output = outputs
            .get("fetch_name_0")
            .ok_or_else(|| {
                OcrError::Recognition("Detection output 'fetch_name_0' not found".to_string())
            })?;

        println!("Reading detection output...");

        // Извлекаем Float32 probability map.
        //
        // Ожидаем:
        //
        // [1, 1, H, W]
        let (output_shape, output_data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| {
                OcrError::Recognition(format!("Failed to extract detection output: {error:?}"))
            })?;

        println!("Detection output shape: {:?}", output_shape);

        println!("Detection output values: {}", output_data.len());

        if output_data.is_empty() {
            return Err(OcrError::Recognition("Detection output tensor is empty".to_string()));
        }

        // Диагностическая статистика.
        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;
        let mut sum = 0.0f64;

        for &value in output_data.iter() {
            min_value = min_value.min(value);
            max_value = max_value.max(value);
            sum += value as f64;
        }

        let mean = sum / (output_data.len() as f64);

        println!("Detection output min: {min_value}");
        println!("Detection output max: {max_value}");
        println!("Detection output mean: {mean}");

        for threshold in [0.1f32, 0.3f32, 0.5f32, 0.7f32, 0.9f32] {
            let count = output_data
                .iter()
                .filter(|&&value| value > threshold)
                .count();

            println!("Values > {}: {} / {}", threshold, count, output_data.len());
        }

        // Output имеет форму [1, 1, H, W].
        let output_height = output_shape[2] as usize;

        let output_width = output_shape[3] as usize;

        // Диагностический preview probability map.
        let threshold = 0.5f32;

        let block_width = 40usize;
        let block_height = 20usize;

        println!("=== DETECTION MAP PREVIEW ===");

        for block_y in 0..block_height {
            let mut line = String::new();

            let y_start = (block_y * output_height) / block_height;

            let y_end = (((block_y + 1) * output_height) / block_height).min(output_height);

            for block_x in 0..block_width {
                let x_start = (block_x * output_width) / block_width;

                let x_end = (((block_x + 1) * output_width) / block_width).min(output_width);

                let mut active = 0usize;
                let mut total = 0usize;

                for y in y_start..y_end {
                    for x in x_start..x_end {
                        let index = y * output_width + x;

                        total += 1;

                        if output_data[index] > threshold {
                            active += 1;
                        }
                    }
                }

                let ratio = if total > 0 { (active as f32) / (total as f32) } else { 0.0 };

                let symbol = if ratio > 0.75 {
                    '#'
                } else if ratio > 0.5 {
                    'O'
                } else if ratio > 0.25 {
                    '+'
                } else if ratio > 0.05 {
                    '.'
                } else {
                    ' '
                };

                line.push(symbol);
            }

            println!("{line}");
        }

        println!("=== END DETECTION MAP PREVIEW ===");

        // DBNet-style postprocess.
        println!("Postprocess input: {}x{}", output_width, output_height);

        let regions = postprocess(output_data, output_width, output_height, 0.5);

        println!("Postprocess regions: {}", regions.len());

        println!("=== PADDLE DETECTION END ===");

        Ok(regions)
    }
}
