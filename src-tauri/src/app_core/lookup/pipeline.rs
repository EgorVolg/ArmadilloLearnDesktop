use std::sync::Arc;

use tauri::{ AppHandle, Emitter };

use crate::app_core::{
    input::event::InputEvent,
    lookup::{
        LookupError,
        image::{ crop_around_point, encode_png, upscale_nearest },
        marker::draw_click_marker,
        prompt::LOOKUP_SYSTEM_PROMPT,
        provider::_trait::AiProvider,
        screenshot::capture_screen,
    },
    overlay::manager::OverlayManager,
};

// Размер области вокруг точки, которую отправляем vision-модели.
const CROP_WIDTH: u32 = 800;
const CROP_HEIGHT: u32 = 600;

// Увеличиваем crop перед отправкой.
const CROP_SCALE: u32 = 2;

// =========================================================
// PIPELINE
// =========================================================

pub struct ClickPipeline {
    app: AppHandle,
    overlay: Arc<OverlayManager>,
    provider: Arc<dyn AiProvider>,
}

impl ClickPipeline {
    // =====================================================
    // NEW
    // =====================================================

    pub fn new(
        overlay: Arc<OverlayManager>,
        app: AppHandle,
        provider: Arc<dyn AiProvider>
    ) -> Self {
        Self {
            app,
            overlay,
            provider,
        }
    }

    // =====================================================
    // PROCESS INPUT EVENT
    // =====================================================

    pub fn process(&self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                println!("ClickPipeline: Lookup at ({x}, {y})");

                match self.lookup(x, y) {
                    Ok(result) => {
                        println!("=== LOOKUP RESULT ===");
                        println!("sentence: {}", result.sentence);
                        println!("word: {}", result.word);
                        println!("sentence_translation: {}", result.sentence_translation);
                        println!("word_translation: {}", result.word_translation);
                        println!("synonyms: {:?}", result.synonyms);
                        println!("part_of_speech: {}", result.part_of_speech);
                        println!("topic: {}", result.topic);
                        println!("=== END LOOKUP RESULT ===");

                        let _ = self.app.emit("lookup-result", result);

                        self.overlay.show(x, y);
                    }

                    Err(error) => {
                        eprintln!("Lookup failed: {error}");

                        let error = LookupError {
                            code: "lookup_failed".to_string(),
                            message: error,
                        };

                        let _ = self.app.emit("lookup-error", error);
                    }
                }
            }
        }
    }

    // =====================================================
    // LOOKUP
    // =====================================================

    fn lookup(
        &self,
        click_x: i32,
        click_y: i32
    ) -> Result<crate::app_core::lookup::types::LookupResult, String> {
        println!("=== LOOKUP START ===");

        // -------------------------------------------------
        // SCREENSHOT
        // -------------------------------------------------

        println!("Capturing full screen...");

        let full_image = capture_screen()?;

        println!("Captured: {}x{}", full_image.width, full_image.height);

        // -------------------------------------------------
        // CLICK MARKER
        // -------------------------------------------------

        let mut marked_image = full_image;

        draw_click_marker(&mut marked_image, click_x, click_y);

        println!("Click marker drawn at ({}, {})", click_x, click_y);

        // -------------------------------------------------
        // CROP AROUND CLICK
        // -------------------------------------------------

        let cropped = crop_around_point(&marked_image, click_x, click_y, CROP_WIDTH, CROP_HEIGHT);

        println!("Vision crop: {}x{}", cropped.width, cropped.height);

        // -------------------------------------------------
        // UPSCALE
        // -------------------------------------------------

        let vision_image = upscale_nearest(&cropped, CROP_SCALE);

        println!("Vision image: {}x{}", vision_image.width, vision_image.height);

        // -------------------------------------------------
        // PNG
        // -------------------------------------------------

        let png = encode_png(&vision_image)?;

        println!("PNG size: {} bytes", png.len());

        // -------------------------------------------------
        // AI PROVIDER
        // -------------------------------------------------

        println!("Sending screenshot to AI provider...");
        let result = self.provider.lookup(&png, LOOKUP_SYSTEM_PROMPT)?;

        println!("=== LOOKUP SUCCESS ===");

        Ok(result)
    }
}
