use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::app_core::{
    lookup::{
        image::{crop_around_point, encode_png, upscale_nearest},
        marker::draw_ocr_highlight,
        prompt::LOOKUP_SYSTEM_PROMPT,
        provider::_trait::AiProvider,
        time::now_ms,
        LookupError,
    },
    ocr::engine::OcrEngine,
};

use tauri::{AppHandle, Emitter};

use crate::app_core::{
    input::event::InputEvent, overlay::manager::OverlayManager, screen::capture::capture_screen,
};

// Размер области вокруг точки, которую отправляем vision-модели.
const CROP_WIDTH: u32 = 350;
const CROP_HEIGHT: u32 = 200;

// Увеличиваем crop перед отправкой.
const CROP_SCALE: u32 = 1;

// =========================================================
// PIPELINE
// =========================================================

pub struct ClickPipeline {
    app: AppHandle,
    overlay: Arc<OverlayManager>,
    provider: Arc<dyn AiProvider>,
    ocr: Arc<Mutex<OcrEngine>>,

    // Флаг «окно-оверлей сейчас показано».
    visible: bool,
}

impl ClickPipeline {
    // =====================================================
    // NEW
    // =====================================================

    pub fn new(
        overlay: Arc<OverlayManager>,
        app: AppHandle,
        provider: Arc<dyn AiProvider>,
        ocr: Arc<Mutex<OcrEngine>>,
    ) -> Self {
        Self {
            app,
            overlay,
            provider,
            ocr,
            visible: false,
        }
    }

    // =====================================================
    // PROCESS INPUT EVENT
    // =====================================================

    pub fn process(&mut self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                let click_at = Instant::now();

                // Если окно-оверлей уже открыт —
                // повторный клик закрывает его сразу.
                if self.visible {
                    self.overlay.hide();
                    self.visible = false;
                } else {
                    match self.lookup(x, y, click_at) {
                        Ok(result) => {
                            let _ = self.app.emit("lookup-result", result);

                            self.overlay.show(x, y);
                            self.visible = true;
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
    }

    // =====================================================
    // LOOKUP
    // =====================================================

    fn lookup(
        &self,
        click_x: i32,
        click_y: i32,
        click_at: Instant,
    ) -> Result<crate::app_core::lookup::types::LookupResult, String> {
        // -------------------------------------------------
        // SCREENSHOT
        // -------------------------------------------------

        let mut captured = capture_screen(click_x, click_y)?;

        // -------------------------------------------------
        // LOCAL OCR
        // -------------------------------------------------

        let ocr_boxes = {
            let mut ocr = self
                .ocr
                .lock()
                .map_err(|_| "OCR engine mutex poisoned".to_string())?;

            ocr.recognize(&captured.image)
                .map_err(|error| format!("OCR failed: {error:#}"))?
        };

        // -------------------------------------------------
        // FIND OCR BOX UNDER CLICK
        // -------------------------------------------------

        let local_x = captured.click_x as f32;
        let local_y = captured.click_y as f32;

        let mut clicked_ocr_bbox: Option<(f32, f32, f32, f32)> = None;

        let mut clicked_ocr_text: Option<String> = None;

        for ocr_box in &ocr_boxes {
            if ocr_box.contains_point(local_x, local_y) {
                clicked_ocr_text = Some(ocr_box.text.clone());
                break;
            }
        }

        match &clicked_ocr_text {
            Some(word) => { 
                println!("CLICKED WORD: {}", word);
            }
            None => {
                println!("CLICKED WORD: <none>");
            }
        }

        // -------------------------------------------------
        // RESULT OF OCR CLICK TEST
        // -------------------------------------------------

        // -------------------------------------------------
        // HIGHLIGHT CLICKED OCR TEXT ON SCREENSHOT
        // -------------------------------------------------

        if let Some(ocr_box) = ocr_boxes
            .iter()
            .find(|ocr_box| ocr_box.contains_point(local_x, local_y))
        {
            let (min_x, min_y, max_x, max_y) = ocr_box.bounding_rect();

            draw_ocr_highlight(&mut captured.image, min_x, min_y, max_x, max_y);
        } else {
            println!("No OCR text under click, no highlight drawn");
        }

        // -------------------------------------------------
        // HIGHLIGHT OCR WORD ON SCREENSHOT
        // -------------------------------------------------
        //
        // OCR уже выполнен по чистому screenshot.
        //

        // -------------------------------------------------
        // CROP AROUND CLICK
        // -------------------------------------------------

        let cropped = crop_around_point(
            &captured.image,
            captured.click_x,
            captured.click_y,
            CROP_WIDTH,
            CROP_HEIGHT,
        );

        // -------------------------------------------------
        // UPSCALE
        // -------------------------------------------------

        let vision_image = upscale_nearest(&cropped, CROP_SCALE);

        // -------------------------------------------------
        // PNG
        // -------------------------------------------------

        let png = encode_png(&vision_image)?;

        if let Some(path) = self.save_debug_screenshot(&png) {
            println!("API screenshot saved to: {}", path.display());
        }

        // -------------------------------------------------
        // AI PROVIDER
        // -------------------------------------------------

        let request_started = Instant::now();

        let result = self.provider.lookup(&png, LOOKUP_SYSTEM_PROMPT)?;

        let received_at = now_ms();

        println!(
            ">>> Time from click to AI response: {:.2} s",
            click_at.elapsed().as_secs_f64()
        );

        Ok(result)
    }

    // =====================================================
    // DEBUG
    // =====================================================

    fn save_debug_screenshot(&self, png: &[u8]) -> Option<PathBuf> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let project_root = manifest_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let dir = project_root.join("screenshots");

        std::fs::create_dir_all(&dir).ok()?;

        let path = dir.join(format!("screenshot_{}.png", now_ms()));

        std::fs::write(&path, png).ok()?;

        Some(path)
    }
}
