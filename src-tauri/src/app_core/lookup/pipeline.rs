use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::app_core::{
    input::event::InputEvent,
    lookup::{
        image::{crop_around_point, encode_png, upscale_nearest},
        prompt::LOOKUP_SYSTEM_PROMPT,
        provider::_trait::AiProvider,
        time::now_ms,
        LookupError,
    },
    ocr::engine::OcrEngine,
    overlay::manager::OverlayManager,
    screen::capture::capture_screen,
};

use tauri::{AppHandle, Emitter};

// Размер области вокруг точки, которую отправляем Vision AI.
const CROP_WIDTH: u32 = 350;
const CROP_HEIGHT: u32 = 200;

// Увеличение изображения перед отправкой.
const CROP_SCALE: u32 = 1;

pub struct ClickPipeline {
    app: AppHandle,
    overlay: Arc<OverlayManager>,
    provider: Arc<dyn AiProvider>,
    ocr: Arc<Mutex<OcrEngine>>,

    // Открыт ли сейчас overlay.
    visible: bool,
}

impl ClickPipeline {
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

    pub fn process(&mut self, event: InputEvent) {
        match event {
            InputEvent::Lookup { x, y } => {
                let click_at = Instant::now();

                // Повторный клик закрывает overlay.
                if self.visible {
                    self.overlay.hide();
                    self.visible = false;
                    return;
                }

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

    fn lookup(
        &self,
        click_x: i32,
        click_y: i32,
        click_at: Instant,
    ) -> Result<crate::app_core::lookup::types::LookupResult, String> {
        // -------------------------------------------------
        // SCREENSHOT
        // -------------------------------------------------

        let captured = capture_screen(click_x, click_y)?;

        // -------------------------------------------------
        // OCR
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
        // FIND WORD UNDER CLICK
        // -------------------------------------------------

        let local_x = captured.click_x as f32;
        let local_y = captured.click_y as f32;

        let clicked_word = ocr_boxes
            .iter()
            .find(|ocr_box| ocr_box.contains_point(local_x, local_y))
            .map(|ocr_box| ocr_box.text.trim().to_string())
            .filter(|word| !word.is_empty());

        match &clicked_word {
            Some(word) => {
                println!("CLICKED WORD: {}", word);
            }

            None => {
                println!("CLICKED WORD: <none>");
            }
        }

        // -------------------------------------------------
        // CROP
        // -------------------------------------------------

        let cropped = crop_around_point(
            &captured.image,
            captured.click_x,
            captured.click_y,
            CROP_WIDTH,
            CROP_HEIGHT,
        );

        let vision_image = upscale_nearest(&cropped, CROP_SCALE);

        // -------------------------------------------------
        // PNG
        // -------------------------------------------------

        let png = encode_png(&vision_image)?;

        if let Some(path) = self.save_debug_screenshot(&png) {
            println!("API screenshot saved to: {}", path.display());
        }

        // -------------------------------------------------
        // AI
        // -------------------------------------------------

        let result = self.provider.lookup(&png, LOOKUP_SYSTEM_PROMPT)?;

        println!(
            ">>> Time from click to AI response: {:.2} s",
            click_at.elapsed().as_secs_f64()
        );

        // -------------------------------------------------
        // IMPORTANT
        // -------------------------------------------------
        //
        // Сейчас AI ещё возвращает полный LookupResult.
        //
        // Но если OCR нашёл слово, именно оно является
        // словом, выбранным пользователем.
        //
        // Поэтому заменяем result.word на OCR-слово.
        //
        // В дальнейшем здесь будет:
        //
        // OCR word
        //     ↓
        // offline dictionary
        //     ↓
        // найдено → без AI
        //     ↓
        // не найдено → AI
        //
        // -------------------------------------------------

        let mut result = result;

        if let Some(word) = clicked_word {
            result.word = word;
        }

        Ok(result)
    }

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
