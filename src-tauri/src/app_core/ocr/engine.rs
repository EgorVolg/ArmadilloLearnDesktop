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

        let boxes = result
            .lines
            .into_iter()
            .map(|line| {
                let points = line.bbox.points;

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
                    confidence: line.score,
                    text: line.text,
                }
            })
            .collect::<Vec<_>>();

        Ok(boxes)
    }
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
