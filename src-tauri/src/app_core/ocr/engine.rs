use anyhow::{Context, Result};

use image::RgbImage;
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

    /// Runs OCR exactly once over the supplied image.
    ///
    /// PP-OCR gives us line-level bounding boxes, so we derive
    /// approximate word-level boxes from the recognized text.
    pub fn recognize(&mut self, image: &Image) -> Result<Vec<OcrBox>> {
        let rgb = image_to_rgb_image(image)?;

        let result = self
            .engine
            .run_image(&rgb)
            .context("PP-OCRv5 OCR inference failed")?;

        let mut boxes = Vec::new();

        for line in result.lines {
            let text = line.text.trim();

            if text.is_empty() {
                continue;
            }

            append_word_boxes(&mut boxes, text, line.bbox.points, line.score);
        }

        Ok(boxes)
    }
}

/// Converts one line-level OCR result into word-level boxes.
///
/// The OCR engine gives us one polygon for the entire line. We cannot
/// obtain true character boxes from that result, so the best we can do
/// is estimate word positions from the horizontal layout.
///
/// Unlike the previous implementation, this function:
///
/// - works entirely with Unicode characters;
/// - accounts for whitespace;
/// - does not perform another OCR inference;
/// - keeps punctuation attached to its word;
/// - gives spaces their own estimated width instead of silently
///   assigning them to the following word.
fn append_word_boxes(boxes: &mut Vec<OcrBox>, text: &str, points: [[f32; 2]; 4], confidence: f32) {
    let words = split_words(text);

    if words.is_empty() {
        return;
    }

    // A single OCR word gets the complete line polygon.
    if words.len() == 1 {
        boxes.push(make_box(points, confidence, words[0].text.to_string()));

        return;
    }

    let total_chars = text.chars().count();

    if total_chars == 0 {
        return;
    }

    /*
     * We estimate the horizontal position of each word using character
     * widths.
     *
     * Example:
     *
     *   "The rain stopped."
     *
     * becomes approximately:
     *
     *   |---The---| |--rain--| |----stopped.----|
     *
     * instead of simply dividing the whole bbox into equal pieces.
     */
    let mut cursor = 0usize;

    for word in words {
        // Find where this word starts in character coordinates.
        //
        // `word.start` is already a character index, not a byte index.
        cursor = word.start;

        let start_ratio = cursor as f32 / total_chars as f32;
        let end_ratio = word.end as f32 / total_chars as f32;

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
            confidence,
            text: word.text.to_string(),
        });

        cursor = word.end;
    }
}

struct Word<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

/// Splits text into words while preserving their character positions.
///
/// Byte offsets are used only for slicing the UTF-8 string.
/// `start`/`end` are character positions and are therefore safe to
/// use for bbox calculations.
fn split_words(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();

    let mut word_start_byte: Option<usize> = None;
    let mut word_start_char: usize = 0;

    for (char_index, (byte_index, ch)) in text.char_indices().enumerate() {
        if ch.is_whitespace() {
            if let Some(start_byte) = word_start_byte.take() {
                words.push(Word {
                    start: word_start_char,
                    end: char_index,
                    text: &text[start_byte..byte_index],
                });
            }
        } else if word_start_byte.is_none() {
            word_start_byte = Some(byte_index);
            word_start_char = char_index;
        }
    }

    if let Some(start_byte) = word_start_byte {
        words.push(Word {
            start: word_start_char,
            end: text.chars().count(),
            text: &text[start_byte..],
        });
    }

    words
}

fn make_box(points: [[f32; 2]; 4], confidence: f32, text: String) -> OcrBox {
    OcrBox {
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
        confidence,
        text,
    }
}

fn interpolate(a: [f32; 2], b: [f32; 2], ratio: f32) -> (f32, f32) {
    (a[0] + (b[0] - a[0]) * ratio, a[1] + (b[1] - a[1]) * ratio)
}

fn image_to_rgb_image(image: &Image) -> Result<RgbImage> {
    let expected_len = image.width as usize * image.height as usize * 3;

    if image.data.len() != expected_len {
        anyhow::bail!(
            "invalid RGB image buffer: expected {} bytes, got {}",
            expected_len,
            image.data.len()
        );
    }

    RgbImage::from_raw(image.width, image.height, image.data.clone())
        .context("failed to construct RgbImage from captured screen")
}
