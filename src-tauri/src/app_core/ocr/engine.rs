use anyhow::{Context, Result};
use rapidocr_core::{config::PipelineConfig, model::PPOCRV5_EN_MOBILE, RapidOcr};

use crate::app_core::lookup::image::Image;

use super::types::{OcrBox, OcrPoint};

pub struct OcrEngine {
    engine: RapidOcr,
}

impl OcrEngine {
    pub fn new(model_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let model_dir = model_dir.into();

        let config = PPOCRV5_EN_MOBILE
            .config(model_dir)
            .with_pipeline(PipelineConfig::full());

        let engine =
            RapidOcr::new(config).context("failed to initialize PP-OCRv5 English OCR engine")?;

        Ok(Self { engine })
    }

    pub fn recognize(&mut self, image: &Image) -> Result<Vec<OcrBox>> {
        let rgb = image_to_rgb_image(image)?;

        let result = self
            .engine
            .run_image(&rgb)
            .context("PP-OCRv5 OCR inference failed")?;

        let mut boxes = Vec::new();

        for line in result.lines {
            let points = line.bbox.points;

            let text = line.text.trim();

            if text.is_empty() {
                continue;
            }

            // PP-OCRv5 даёт bbox всей строки.
            //
            // Разбиваем строку на отдельные слова и создаём
            // приблизительные word-level bbox внутри исходного bbox.
            //
            // Это позволяет определить именно слово под курсором,
            // не отправляя в pipeline всю строку целиком.

            let words: Vec<(usize, usize, &str)> = {
                let mut words = Vec::new();
                let mut word_start: Option<usize> = None;

                for (index, ch) in text.char_indices() {
                    if ch.is_whitespace() {
                        if let Some(start) = word_start.take() {
                            words.push((start, index, &text[start..index]));
                        }
                    } else if word_start.is_none() {
                        word_start = Some(index);
                    }
                }

                if let Some(start) = word_start {
                    words.push((start, text.len(), &text[start..]));
                }

                words
            };

            if words.len() <= 1 {
                boxes.push(OcrBox {
                    points: [
                        OcrPoint {
                            x: points[0][0],
                            y: points[0][1],
                        },
                        OcrPoint {
                            x: points[1][0],
                            y: points[1][1],
                        },
                        OcrPoint {
                            x: points[2][0],
                            y: points[2][1],
                        },
                        OcrPoint {
                            x: points[3][0],
                            y: points[3][1],
                        },
                    ],
                    confidence: line.score,
                    text: text.to_string(),
                });

                continue;
            }

            let total_chars = text.chars().count() as f32;

            if total_chars <= 0.0 {
                continue;
            }

            let mut char_position = 0usize;

            for (_, _, word) in words {
                let leading_chars = text
                    .chars()
                    .skip(char_position)
                    .take_while(|ch| ch.is_whitespace())
                    .count();

                char_position += leading_chars;

                let word_chars = word.chars().count();

                if word_chars == 0 {
                    continue;
                }

                let start_ratio = char_position as f32 / total_chars;
                let end_ratio = (char_position + word_chars) as f32 / total_chars;

                let top_left = interpolate(points[0], points[1], start_ratio);
                let top_right = interpolate(points[0], points[1], end_ratio);
                let bottom_right = interpolate(points[3], points[2], end_ratio);
                let bottom_left = interpolate(points[3], points[2], start_ratio);

                boxes.push(OcrBox {
                    points: [
                        OcrPoint {
                            x: top_left.0,
                            y: top_left.1,
                        },
                        OcrPoint {
                            x: top_right.0,
                            y: top_right.1,
                        },
                        OcrPoint {
                            x: bottom_right.0,
                            y: bottom_right.1,
                        },
                        OcrPoint {
                            x: bottom_left.0,
                            y: bottom_left.1,
                        },
                    ],
                    confidence: line.score,
                    text: word.to_string(),
                });

                char_position += word_chars;
            }
        }

        Ok(boxes)
    }
}

fn interpolate(a: [f32; 2], b: [f32; 2], ratio: f32) -> (f32, f32) {
    (a[0] + (b[0] - a[0]) * ratio, a[1] + (b[1] - a[1]) * ratio)
}

fn image_to_rgb_image(image: &Image) -> Result<image::RgbImage> {
    let expected_len = image.width as usize * image.height as usize * 3;

    if image.data.len() != expected_len {
        anyhow::bail!(
            "invalid RGB image buffer: expected {} bytes, got {}",
            expected_len,
            image.data.len()
        );
    }

    image::RgbImage::from_raw(image.width, image.height, image.data.clone())
        .context("failed to construct RgbImage from captured screen")
}
