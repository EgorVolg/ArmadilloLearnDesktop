use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

use crate::app_core::{
    input::event::InputEvent,
    lookup::{provider::_trait::AiProvider, time::now_ms, LookupError},
    ocr::{engine::OcrEngine, types::OcrBox},
    overlay::manager::OverlayManager,
    screen::capture::capture_screen,
};

use tauri::{AppHandle, Emitter};

pub struct ClickPipeline {
    app: AppHandle,
    overlay: Arc<OverlayManager>,
    provider: Arc<dyn AiProvider>,
    ocr: Arc<Mutex<OcrEngine>>,
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
                println!("ClickPipeline: Lookup at ({x}, {y})");

                let click_at = Instant::now();

                // Second click closes the overlay.
                if self.visible {
                    println!("Overlay is open, closing it immediately");
                    self.overlay.hide();
                    self.visible = false;
                    return;
                }

                match self.lookup(x, y, click_at) {
                    Ok((result, _clicked_ocr_bbox)) => {
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

    fn lookup(
        &self,
        click_x: i32,
        click_y: i32,
        click_at: Instant,
    ) -> Result<
        (
            crate::app_core::lookup::types::LookupResult,
            Option<(f32, f32, f32, f32)>,
        ),
        String,
    > {
        println!("=== LOOKUP START ===");
        println!("Global click: ({click_x}, {click_y})");

        // =========================================================
        // SCREENSHOT
        // =========================================================

        println!("Capturing monitor under click...");

        let captured = capture_screen(click_x, click_y)?;

        println!(
            "Captured monitor: {}x{}, origin=({}, {})",
            captured.image.width, captured.image.height, captured.origin_x, captured.origin_y
        );

        println!(
            "Click in screenshot coordinates: ({}, {})",
            captured.click_x, captured.click_y
        );

        // =========================================================
        // OCR
        // =========================================================

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

        if ocr_boxes.is_empty() {
            return Err("OCR detected no text".to_string());
        }

        // =========================================================
        // FIND CLICKED OCR BOX
        // =========================================================

        let local_x = captured.click_x as f32;
        let local_y = captured.click_y as f32;

        println!(
            "Searching OCR boxes for click at ({:.1}, {:.1})",
            local_x, local_y
        );

        let clicked_index = ocr_boxes
            .iter()
            .enumerate()
            .filter(|(_, ocr_box)| ocr_box.contains_point(local_x, local_y))
            .min_by(|(_, a), (_, b)| {
                bbox_area(a)
                    .partial_cmp(&bbox_area(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
            .ok_or_else(|| "No OCR text region found under cursor".to_string())?;

        let clicked_ocr = &ocr_boxes[clicked_index];

        let clicked_ocr_local_bbox = clicked_ocr.bounding_rect();

        println!(
            "Clicked OCR LOCAL bbox=({:.1},{:.1})-({:.1},{:.1})",
            clicked_ocr_local_bbox.0,
            clicked_ocr_local_bbox.1,
            clicked_ocr_local_bbox.2,
            clicked_ocr_local_bbox.3
        );

        let clicked_word = clicked_ocr.text.trim().to_string();

        if clicked_word.is_empty() {
            return Err("OCR found an empty word under cursor".to_string());
        }

        println!(
            "Clicked OCR index={} text='{}'",
            clicked_index, clicked_word
        );

        // =========================================================
        // CLICKED WORD BBOX
        // =========================================================

        let clicked_ocr_bbox = {
            let (min_x, min_y, max_x, max_y) = clicked_ocr.bounding_rect();

            Some((
                min_x + captured.origin_x as f32,
                min_y + captured.origin_y as f32,
                max_x + captured.origin_x as f32,
                max_y + captured.origin_y as f32,
            ))
        };

        if let Some(bbox) = clicked_ocr_bbox {
            println!(
                "CLICKED WORD: '{}' bbox=({:.1}, {:.1})-({:.1}, {:.1})",
                clicked_word, bbox.0, bbox.1, bbox.2, bbox.3
            );
        }

        // =========================================================
        // DEBUG OCR
        // =========================================================

        debug_ocr_boxes(&ocr_boxes, clicked_index, local_x, local_y);

        // =========================================================
        // BUILD SMALL SENTENCE CONTEXT
        // =========================================================

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        println!("FINAL OCR CONTEXT: '{}'", context);

        if context.trim().is_empty() {
            return Err("Failed to build OCR context".to_string());
        }

        // =========================================================
        // AI PROVIDER
        // =========================================================

        let sent_at = now_ms();

        println!(
            "Sending text to AI provider... word='{}', sent at {} ms",
            clicked_word, sent_at
        );

        let request_started = Instant::now();

        let result = self.provider.lookup(&context, &clicked_word)?;

        let received_at = now_ms();

        println!(
            "AI provider responded: received at {} ms, round-trip took {} ms",
            received_at,
            request_started.elapsed().as_millis()
        );

        println!(
            ">>> Time from click to AI response: {:.2} s",
            click_at.elapsed().as_secs_f64()
        );

        println!("=== LOOKUP SUCCESS ===");

        Ok((result, clicked_ocr_bbox))
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

// ============================================================================
// SENTENCE CONTEXT
// ============================================================================
//
// Контекст специально ограничен.
//
// Мы НЕ берём всю OCR-строку.
// Берём только небольшой участок вокруг clicked word:
//
//   LEFT_CONTEXT_WORDS  = сколько слов слева
//   RIGHT_CONTEXT_WORDS = сколько слов справа
//
// Дополнительно контекст обрывается:
//   - на конце предложения;
//   - на большом горизонтальном разрыве;
//   - при слишком большой суммарной ширине.
//
// Это существенно уменьшает шанс отправить в AI соседние UI-блоки.
// ============================================================================

const LEFT_CONTEXT_WORDS: usize = 5;
const RIGHT_CONTEXT_WORDS: usize = 7;

// Максимальная горизонтальная ширина контекста относительно
// clicked word. Это дополнительный предохранитель от огромных строк.
const MAX_CONTEXT_WIDTH_MULTIPLIER: f32 = 14.0;

// Минимальная ширина, чтобы на маленьких шрифтах ограничение
// не стало слишком агрессивным.
const MIN_CONTEXT_WIDTH: f32 = 300.0;

fn extract_sentence_context(ocr_boxes: &[OcrBox], clicked_index: usize) -> String {
    if ocr_boxes.is_empty() {
        return String::new();
    }

    if clicked_index >= ocr_boxes.len() {
        return String::new();
    }

    let clicked = &ocr_boxes[clicked_index];

    let line = collect_same_visual_line(ocr_boxes, clicked_index);

    if line.is_empty() {
        return clicked.text.trim().to_string();
    }

    println!("Same visual line contains {} OCR boxes", line.len());

    let clicked_position = line.iter().position(|index| *index == clicked_index);

    let Some(clicked_position) = clicked_position else {
        return clicked.text.trim().to_string();
    };

    println!(
        "Clicked box position inside visual line: {}",
        clicked_position
    );

    // =========================================================
    // CONTEXT WIDTH
    // =========================================================

    let (clicked_min_x, _clicked_min_y, clicked_max_x, _clicked_max_y) = clicked.bounding_rect();

    let clicked_width = (clicked_max_x - clicked_min_x).max(1.0);

    let max_context_width = (clicked_width * MAX_CONTEXT_WIDTH_MULTIPLIER).max(MIN_CONTEXT_WIDTH);

    // =========================================================
    // EXPAND LEFT
    // =========================================================

    let mut left = clicked_position;

    let mut left_words = 0usize;

    while left > 0 && left_words < LEFT_CONTEXT_WORDS {
        let current_index = line[left];
        let previous_index = line[left - 1];

        let current = &ocr_boxes[current_index];
        let previous = &ocr_boxes[previous_index];

        // Предыдущий OCR box завершает предложение.
        if ends_sentence(previous.text.trim()) {
            break;
        }

        // Большой горизонтальный разрыв = другой блок.
        if is_large_horizontal_gap(previous, current) {
            break;
        }

        // Проверяем общую ширину будущего контекста.
        let candidate_left = previous.bounding_rect().0;

        let candidate_width = clicked_max_x - candidate_left;

        if candidate_width > max_context_width {
            break;
        }

        left -= 1;
        left_words += 1;
    }

    // =========================================================
    // EXPAND RIGHT
    // =========================================================

    let mut right = clicked_position;

    let mut right_words = 0usize;

    while right + 1 < line.len() && right_words < RIGHT_CONTEXT_WORDS {
        let current_index = line[right];
        let next_index = line[right + 1];

        let current = &ocr_boxes[current_index];
        let next = &ocr_boxes[next_index];

        // Большой горизонтальный разрыв = другой блок.
        if is_large_horizontal_gap(current, next) {
            break;
        }

        let candidate_right = next.bounding_rect().2;

        let candidate_width = candidate_right - clicked_min_x;

        if candidate_width > max_context_width {
            break;
        }

        right += 1;
        right_words += 1;

        // После добавления box проверяем конец предложения.
        if ends_sentence(next.text.trim()) {
            break;
        }
    }

    // =========================================================
    // BUILD FINAL CONTEXT
    // =========================================================

    let mut parts = Vec::new();

    for position in left..=right {
        let index = line[position];

        let text = ocr_boxes[index].text.trim();

        if !text.is_empty() {
            parts.push(text);
        }
    }

    let context = join_ocr_text(&parts);

    println!("Context range: line[{}..={}]", left, right);

    println!("Context built from {} OCR boxes", parts.len());

    println!(
        "Context limits: left={}, right={}, max_width={:.1}px",
        LEFT_CONTEXT_WORDS, RIGHT_CONTEXT_WORDS, max_context_width
    );

    context
}

// ============================================================================
// SAME VISUAL LINE
// ============================================================================

fn collect_same_visual_line(ocr_boxes: &[OcrBox], clicked_index: usize) -> Vec<usize> {
    if clicked_index >= ocr_boxes.len() {
        return Vec::new();
    }

    let clicked = &ocr_boxes[clicked_index];

    let (_clicked_min_x, clicked_min_y, _clicked_max_x, clicked_max_y) = clicked.bounding_rect();

    let clicked_height = (clicked_max_y - clicked_min_y).max(1.0);

    let clicked_center_y = (clicked_min_y + clicked_max_y) / 2.0;

    // Было 0.65.
    //
    // Немного уменьшаем tolerance, чтобы соседние строки
    // не слипались в одну "визуальную строку".
    let tolerance = (clicked_height * 0.45).max(6.0);

    let mut indexes = Vec::new();

    for (index, item) in ocr_boxes.iter().enumerate() {
        if same_visual_line(
            item,
            clicked_center_y,
            clicked_min_y,
            clicked_max_y,
            tolerance,
        ) {
            indexes.push(index);
        }
    }

    // Left -> right.
    indexes.sort_by(|a, b| {
        let ax = ocr_boxes[*a].bounding_rect().0;
        let bx = ocr_boxes[*b].bounding_rect().0;

        ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
    });

    indexes
}

fn same_visual_line(
    item: &OcrBox,
    clicked_center_y: f32,
    clicked_min_y: f32,
    clicked_max_y: f32,
    tolerance: f32,
) -> bool {
    let (_, min_y, _, max_y) = item.bounding_rect();

    let center_y = (min_y + max_y) / 2.0;

    let vertical_distance = (center_y - clicked_center_y).abs();

    let vertical_overlap = min_y <= clicked_max_y && max_y >= clicked_min_y;

    vertical_overlap || vertical_distance <= tolerance
}

// ============================================================================
// HORIZONTAL GAP
// ============================================================================

fn is_large_horizontal_gap(left: &OcrBox, right: &OcrBox) -> bool {
    let (_left_min_x, left_min_y, left_max_x, left_max_y) = left.bounding_rect();

    let (right_min_x, right_min_y, _right_max_x, right_max_y) = right.bounding_rect();

    let gap = right_min_x - left_max_x;

    if gap <= 0.0 {
        return false;
    }

    let left_height = (left_max_y - left_min_y).abs();

    let right_height = (right_max_y - right_min_y).abs();

    let text_height = left_height.max(right_height).max(1.0);

    // Было 1.5.
    //
    // Для ограничения контекста лучше чуть раньше
    // считать большой gap границей блока.
    let threshold = text_height * 1.25;

    gap >= threshold
}

// ============================================================================
// OCR TEXT JOINING
// ============================================================================

fn join_ocr_text(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// SENTENCE END
// ============================================================================

fn ends_sentence(text: &str) -> bool {
    let text = text.trim();

    if text.is_empty() {
        return false;
    }

    let without_closing =
        text.trim_end_matches(|c: char| matches!(c, '"' | '\'' | ')' | ']' | '}'));

    without_closing.ends_with('.')
        || without_closing.ends_with('?')
        || without_closing.ends_with('!')
}

// ============================================================================
// DEBUG OCR
// ============================================================================

fn debug_ocr_boxes(ocr_boxes: &[OcrBox], clicked_index: usize, local_x: f32, local_y: f32) {
    println!(
        "=== OCR DEBUG === cursor=({:.1}, {:.1}) clicked_index={}",
        local_x, local_y, clicked_index
    );

    for (index, item) in ocr_boxes.iter().enumerate() {
        let (min_x, min_y, max_x, max_y) = item.bounding_rect();

        let clicked = index == clicked_index;

        let contains = item.contains_point(local_x, local_y);

        println!(
            "OCR #{index}: '{}' confidence={:.3} \
             bbox=({:.1},{:.1})-({:.1},{:.1}) \
             contains_click={} clicked={}",
            item.text, item.confidence, min_x, min_y, max_x, max_y, contains, clicked
        );
    }

    println!("=== END OCR DEBUG ===");
}

// ============================================================================
// BBOX
// ============================================================================

fn bbox_area(ocr_box: &OcrBox) -> f32 {
    let (min_x, min_y, max_x, max_y) = ocr_box.bounding_rect();

    let width = (max_x - min_x).max(0.0);

    let height = (max_y - min_y).max(0.0);

    width * height
}
