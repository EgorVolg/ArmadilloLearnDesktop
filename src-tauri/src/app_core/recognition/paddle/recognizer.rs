use std::sync::Mutex;

use ort::{ session::Session, value::Tensor };

use crate::app_core::recognition::{ image::Image, region::Region, types::{ OcrError, TextRegion } };

pub struct PaddleRecognizer {
    session: Mutex<Session>,
    dictionary: Vec<String>,
}

impl PaddleRecognizer {
    pub fn new() -> Result<Self, OcrError> {
        println!("=== INITIALIZING PADDLE RECOGNIZER ===");

        let model_path = "models/rec/inference.onnx";
        let dict_path = "models/rec/ppocrv5_latin_dict.txt";

        println!("Loading recognition model: {model_path}");

        let session = Session::builder()
            .map_err(|e| {
                OcrError::Recognition(format!("Failed to create recognition session: {e:?}"))
            })?
            .with_memory_pattern(false)
            .map_err(|e| {
                OcrError::Recognition(format!("Failed to configure recognition session: {e:?}"))
            })?
            .commit_from_file(model_path)
            .map_err(|e| {
                OcrError::Recognition(format!("Failed to load recognition model: {e:?}"))
            })?;

        println!("Recognition model loaded.");
        println!("Inputs: {}", session.inputs().len());
        println!("Outputs: {}", session.outputs().len());

        let dictionary = std::fs
            ::read_to_string(dict_path)
            .map_err(|e| { OcrError::Recognition(format!("Failed to read dictionary: {e}")) })?
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        println!("Recognition dictionary loaded: {} symbols", dictionary.len());

        Ok(Self {
            session: Mutex::new(session),
            dictionary,
        })
    }

    /// Распознаёт текст во всех найденных detection regions.
    ///
    /// На вход:
    /// - исходное cropped изображение;
    /// - Vec<Region>, найденных detector/postprocess.
    ///
    /// На выход:
    /// - Vec<TextRegion>.
    pub fn recognize(
        &self,
        image: &Image,
        regions: Vec<Region>
    ) -> Result<Vec<TextRegion>, OcrError> {
        let mut results = Vec::with_capacity(regions.len());

        for (index, region) in regions.into_iter().enumerate() {
            println!("=== RECOGNITION REGION #{} ===", index);

            println!(
                "Region: x={}, y={}, width={}, height={}",
                region.x,
                region.y,
                region.width,
                region.height
            );

            let crop = crop_region(image, region);

            if crop.width == 0 || crop.height == 0 {
                println!("Skipping empty region.");

                results.push(TextRegion {
                    text: String::new(),
                    confidence: 0.0,
                    region,
                });

                continue;
            }

            let (data, height, width) = preprocess_recognition(&crop);

            let shape = [1usize, 3usize, height, width];

            let tensor = Tensor::from_array((shape, data)).map_err(|e| {
                OcrError::Engine(format!("Failed to create recognition tensor: {e}"))
            })?;

            let mut session = self.session
                .lock()
                .map_err(|e| {
                    OcrError::Recognition(format!("Failed to lock recognition session: {e}"))
                })?;

            println!("Starting recognition inference...");

            let outputs = session
                .run(ort::inputs![tensor])
                .map_err(|e| {
                    OcrError::Recognition(format!("Recognition inference failed: {e:?}"))
                })?;

            println!("Recognition inference completed. Outputs: {}", outputs.len());

            let output = outputs
                .values()
                .next()
                .ok_or_else(|| {
                    OcrError::Recognition("Recognition output not found".to_string())
                })?;

            let (shape, data) = output
                .try_extract_tensor::<f32>()
                .map_err(|e| {
                    OcrError::Recognition(format!("Failed to extract recognition output: {e:?}"))
                })?;

            println!("Recognition output shape: {:?}", shape);

            let (text, confidence) = decode_ctc(shape, data, &self.dictionary)?;

            println!("Recognized: '{}' confidence={:.3}", text, confidence);

            results.push(TextRegion {
                text,
                confidence,
                region,
            });
        }

        Ok(results)
    }
}

fn crop_region(image: &Image, region: Region) -> Image {
    crate::app_core::recognition::crop::crop(image, region).unwrap_or_else(|_| Image {
        width: 0,
        height: 0,
        data: Vec::new(),
    })
}

/// Подготовка изображения для recognition.
///
/// Сейчас используем фиксированный input:
///
/// [1, 3, 48, 320]
fn preprocess_recognition(image: &Image) -> (Vec<f32>, usize, usize) {
    const HEIGHT: usize = 48;
    const WIDTH: usize = 320;

    let mut data = vec![0.0f32; 3 * HEIGHT * WIDTH];

    if image.width == 0 || image.height == 0 {
        return (data, HEIGHT, WIDTH);
    }

    let src_width = image.width as usize;
    let src_height = image.height as usize;

    let scale = ((WIDTH as f32) / (src_width as f32)).min((HEIGHT as f32) / (src_height as f32));

    let new_width = (((src_width as f32) * scale).round() as usize).clamp(1, WIDTH);

    let new_height = (((src_height as f32) * scale).round() as usize).clamp(1, HEIGHT);

    for y in 0..new_height {
        for x in 0..new_width {
            let sx = (x * src_width) / new_width;
            let sy = (y * src_height) / new_height;

            let src = (sy * src_width + sx) * 3;

            let r = (image.data[src] as f32) / 255.0;

            let g = (image.data[src + 1] as f32) / 255.0;

            let b = (image.data[src + 2] as f32) / 255.0;

            let dst = y * WIDTH + x;

            data[dst] = (r - 0.5) / 0.5;

            data[HEIGHT * WIDTH + dst] = (g - 0.5) / 0.5;

            data[2 * HEIGHT * WIDTH + dst] = (b - 0.5) / 0.5;
        }
    }

    (data, HEIGHT, WIDTH)
}

fn decode_ctc(
    shape: &[i64],
    data: &[f32],
    dictionary: &[String]
) -> Result<(String, f32), OcrError> {
    if shape.len() != 3 {
        return Err(
            OcrError::Recognition(format!("Unexpected recognition output shape: {:?}", shape))
        );
    }

    let time = shape[1] as usize;
    let classes = shape[2] as usize;

    if time == 0 || classes == 0 {
        return Ok((String::new(), 0.0));
    }

    let mut text = String::new();

    let mut confidence_sum = 0.0f32;
    let mut confidence_count = 0usize;

    let mut previous = usize::MAX;

    for t in 0..time {
        let offset = t * classes;

        let mut best_class = 0usize;
        let mut best_score = f32::NEG_INFINITY;

        for c in 0..classes {
            let score = data[offset + c];

            if score > best_score {
                best_score = score;
                best_class = c;
            }
        }

        // CTC blank.
        if best_class == 0 {
            previous = best_class;
            continue;
        }

        // Убираем повторяющиеся символы.
        if best_class == previous {
            continue;
        }

        let dictionary_index = best_class - 1;

        if dictionary_index < dictionary.len() {
            text.push_str(&dictionary[dictionary_index]);

            confidence_sum += best_score;
            confidence_count += 1;
        }

        previous = best_class;
    }

    let confidence = if confidence_count > 0 {
        confidence_sum / (confidence_count as f32)
    } else {
        0.0
    };

    Ok((text, confidence))
}
