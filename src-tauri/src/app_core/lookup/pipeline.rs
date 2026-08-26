use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::app_core::{
    lookup::{
        image::{crop_around_point, encode_png, upscale_nearest},
        marker::draw_click_marker,
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
                println!("ClickPipeline: Lookup at ({x}, {y})");

                let click_at = Instant::now();

                // Если окно-оверлей уже открыто —
                // повторный клик закрывает его сразу.
                if self.visible {
                    println!("Overlay is open, closing it immediately");

                    self.overlay.hide();
                    self.visible = false;
                } else {
                    match self.lookup(x, y, click_at) {
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
        println!("=== LOOKUP START ===");
        println!("Global click: ({click_x}, {click_y})");

        // -------------------------------------------------
        // SCREENSHOT
        // -------------------------------------------------

        println!("Capturing monitor under click...");

        let mut captured = capture_screen(click_x, click_y)?;

        println!(
            "Captured monitor: {}x{}, origin=({}, {})",
            captured.image.width, captured.image.height, captured.origin_x, captured.origin_y
        );

        println!(
            "Click in screenshot coordinates: ({}, {})",
            captured.click_x, captured.click_y
        );

        // -------------------------------------------------
        // LOCAL OCR
        // -------------------------------------------------

        println!("=== STARTING LOCAL OCR ===");

        let ocr_boxes = {
            let mut ocr = self
                .ocr
                .lock()
                .map_err(|_| "OCR engine mutex poisoned".to_string())?;

            ocr.recognize(&captured.image)
                .map_err(|error| format!("OCR failed: {error:#}"))?
        };

        println!("=== LOCAL OCR COMPLETE ===");
        println!("OCR detected {} text regions", ocr_boxes.len());

        // -------------------------------------------------
        // FIND OCR BOX UNDER CLICK
        // -------------------------------------------------

        let local_x = captured.click_x as f32;
        let local_y = captured.click_y as f32;

        println!(
            "Searching OCR boxes for click at ({:.1}, {:.1})",
            local_x, local_y
        );

        let mut clicked_ocr_bbox: Option<(f32, f32, f32, f32)> = None;

        for (index, ocr_box) in ocr_boxes.iter().enumerate() {
            let (min_x, min_y, max_x, max_y) = ocr_box.bounding_rect();

            let contains_click = ocr_box.contains_point(local_x, local_y);

            println!(
                "OCR #{index}: '{}' confidence={:.3} \
                 bbox=({:.1}, {:.1})-({:.1}, {:.1}) \
                 contains_click={}",
                ocr_box.text, ocr_box.confidence, min_x, min_y, max_x, max_y, contains_click
            );

            if contains_click {
                clicked_ocr_bbox = Some((
                    min_x + captured.origin_x as f32,
                    min_y + captured.origin_y as f32,
                    max_x + captured.origin_x as f32,
                    max_y + captured.origin_y as f32,
                ));
            }
        }

        // -------------------------------------------------
        // RESULT OF OCR CLICK TEST
        // -------------------------------------------------

        println!("=== OCR CLICK TEST ===");

        match clicked_ocr_bbox {
            Some((min_x, min_y, max_x, max_y)) => {
                println!(
                    "CLICKED OCR BBOX: ({:.1}, {:.1}) - ({:.1}, {:.1})",
                    min_x, min_y, max_x, max_y
                );
            }

            None => {
                println!("NO OCR BBOX UNDER CLICK");
            }
        }

        println!("=== END OCR CLICK TEST ===");

        // -------------------------------------------------
        // MARK CLICK ON SCREENSHOT
        // -------------------------------------------------
        //
        // ВАЖНО:
        // OCR уже выполнен по чистому screenshot.
        // Теперь добавляем визуальную метку.
        //
        // Именно эта версия изображения пойдёт дальше
        // в crop -> PNG -> Vision AI.
        // -------------------------------------------------

        draw_click_marker(&mut captured.image, captured.click_x, captured.click_y);

        println!(
            "Click marker drawn at screenshot coordinates: ({}, {})",
            captured.click_x, captured.click_y
        );

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

        println!("Vision crop: {}x{}", cropped.width, cropped.height);

        // -------------------------------------------------
        // UPSCALE
        // -------------------------------------------------

        let vision_image = upscale_nearest(&cropped, CROP_SCALE);

        println!(
            "Vision image: {}x{}",
            vision_image.width, vision_image.height
        );

        // -------------------------------------------------
        // PNG
        // -------------------------------------------------

        let png = encode_png(&vision_image)?;

        println!("PNG size: {} bytes", png.len());

        if let Some(path) = self.save_debug_screenshot(&png) {
            println!("API screenshot saved to: {}", path.display());
        }

        // -------------------------------------------------
        // AI PROVIDER
        // -------------------------------------------------

        let sent_at = now_ms();

        println!(
            "Sending screenshot to AI provider... \
             (sent at {sent_at} ms)"
        );

        let request_started = Instant::now();

        let result = self.provider.lookup(&png, LOOKUP_SYSTEM_PROMPT)?;

        let received_at = now_ms();

        println!(
            "AI provider responded: received at {received_at} ms, \
             round-trip took {} ms",
            request_started.elapsed().as_millis()
        );

        println!(
            ">>> Time from click to AI response: {:.2} s",
            click_at.elapsed().as_secs_f64()
        );

        println!("=== LOOKUP SUCCESS ===");

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
