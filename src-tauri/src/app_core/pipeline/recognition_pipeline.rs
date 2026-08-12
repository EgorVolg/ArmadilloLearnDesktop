use crate::app_core::recognition::{
    capture::capture_screen,
    crop::crop,
    paddle::{ preprocess::preprocess, recognizer::PaddleRecognizer, PaddleDetector },
    region::Region,
    types::OcrResult,
};

pub struct RecognitionPipeline {
    detector: PaddleDetector,
    recognizer: PaddleRecognizer,
}

impl RecognitionPipeline {
    pub fn new() -> Result<Self, String> {
        println!("=== INITIALIZING RECOGNITION PIPELINE ===");

        let detector = PaddleDetector::new().map_err(|error| format!("{error:?}"))?;

        let recognizer = PaddleRecognizer::new().map_err(|error| format!("{error:?}"))?;

        println!("=== RECOGNITION PIPELINE READY ===");

        Ok(Self {
            detector,
            recognizer,
        })
    }

    pub fn run(&self, click_x: i32, click_y: i32) -> Result<OcrResult, String> {
        println!("=== RECOGNITION PIPELINE START ===");

        // -------------------------------------------------
        // CAPTURE
        // -------------------------------------------------

        let image = capture_screen().map_err(|error| format!("{error:?}"))?;

        println!("Captured: {}x{}", image.width, image.height);

        // -------------------------------------------------
        // REGION
        // -------------------------------------------------

        let screen_width = image.width as i32;
        let screen_height = image.height as i32;

        let region_width = 800u32;
        let region_height = 400u32;

        let max_x = (screen_width - (region_width as i32)).max(0);

        let max_y = (screen_height - (region_height as i32)).max(0);

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

        // -------------------------------------------------
        // CROP
        // -------------------------------------------------

        let cropped = crop(&image, region).map_err(|error| format!("{error:?}"))?;

        println!("Cropped: {}x{}", cropped.width, cropped.height);

        // -------------------------------------------------
        // DETECTION PREPROCESS
        // -------------------------------------------------

        let input = preprocess(&cropped);

        println!("Detection input: {}x{}", input.width, input.height);

        // -------------------------------------------------
        // DETECTION + POSTPROCESS
        // -------------------------------------------------

        let detected_regions = self.detector.detect(input).map_err(|error| format!("{error:?}"))?;

        println!("Detection inference completed. Regions: {}", detected_regions.len());

        // -------------------------------------------------
        // RECOGNITION
        // -------------------------------------------------

        let regions = self.recognizer
            .recognize(&cropped, detected_regions)
            .map_err(|error| format!("{error:?}"))?;

        // -------------------------------------------------
        // RESULTS
        // -------------------------------------------------

        println!("=== OCR RESULTS ===");

        for (index, region) in regions.iter().enumerate() {
            println!(
                "#{}: '{}' confidence={:.3} \
                 x={} y={} width={} height={}",
                index,
                region.text,
                region.confidence,
                region.region.x,
                region.region.y,
                region.region.width,
                region.region.height
            );
        }

        println!("Recognition completed. Regions: {}", regions.len());

        println!("=== RECOGNITION PIPELINE END ===");

        Ok(OcrResult {
            regions,
        })
    }
}
