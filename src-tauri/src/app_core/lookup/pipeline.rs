use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use tauri::{AppHandle, Emitter};

use crate::app_core::{
    input::event::InputEvent,
    lookup::{
        image::{crop_around_point, encode_png, upscale_nearest},
        marker::draw_click_marker,
        prompt::LOOKUP_SYSTEM_PROMPT,
        provider::_trait::AiProvider,
        screenshot::capture_screen,
        time::now_ms,
        LookupError,
    },
    overlay::manager::OverlayManager,
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
    // Флаг «окно-оверлей сейчас показано». Точкой входа владеет
    // единственный поток-обработчик событий, поэтому поле не шарится между потоками.
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
    ) -> Self {
        Self {
            app,
            overlay,
            provider,
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

                // Засекаем момент клика, чтобы потом залогировать,
                // сколько секунд прошло от клика до выполнения запроса.
                let click_at = Instant::now();

                // Если окно-оверлей уже открыто - повторный клик закрывает
                // его сразу, без повторного распознавания слова.
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

        let captured = capture_screen(click_x, click_y)?;

        println!(
            "Captured monitor: {}x{}, origin=({}, {})",
            captured.image.width, captured.image.height, captured.origin_x, captured.origin_y
        );

        // -------------------------------------------------
        // CONVERT GLOBAL -> LOCAL
        // -------------------------------------------------

        let local_x = click_x - captured.origin_x;
        let local_y = click_y - captured.origin_y;

        println!(
            "Click coordinates: global=({}, {}), local=({}, {})",
            click_x, click_y, local_x, local_y
        );

        // -------------------------------------------------
        // CLICK MARKER
        // -------------------------------------------------

        let mut marked_image = captured.image;

        draw_click_marker(&mut marked_image, local_x, local_y);

        println!(
            "Click marker drawn at local coordinates ({}, {})",
            local_x, local_y
        );

        // -------------------------------------------------
        // CROP AROUND CLICK
        // -------------------------------------------------

        let cropped = crop_around_point(&marked_image, local_x, local_y, CROP_WIDTH, CROP_HEIGHT);

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

        println!("Sending screenshot to AI provider... (sent at {sent_at} ms)");

        let request_started = Instant::now();

        let result = self.provider.lookup(&png, LOOKUP_SYSTEM_PROMPT)?;

        let received_at = now_ms();

        println!(
            "AI provider responded: received at {received_at} ms, round-trip took {} ms",
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
    // DEBUG: сохраняем скриншот, который ушёл в API,
    // чтобы можно было визуально проверить маркер и кадр.
    // Папка лежит в корне проекта: <корень проекта>/screenshots
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
