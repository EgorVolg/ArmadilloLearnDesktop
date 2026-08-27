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
        // BUILD SENTENCE CONTEXT
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
// Context is determined ONLY by:
//
// 1. Visual line.
// 2. Sentence punctuation.
// 3. Large horizontal OCR gaps.
//
// There is deliberately NO dictionary of "bad" words.
//
// "Share", "Open", "Rust", "GitHub", "Run", etc.
// are perfectly valid words and must NOT be removed.
// ============================================================================

fn extract_sentence_context(ocr_boxes: &[OcrBox], clicked_index: usize) -> String {
    if ocr_boxes.is_empty() {
        return String::new();
    }

    if clicked_index >= ocr_boxes.len() {
        return String::new();
    }

    let clicked = &ocr_boxes[clicked_index];

    // ------------------------------------------------------------
    // Find boxes belonging to the same visual line.
    // ------------------------------------------------------------

    let line = collect_same_visual_line(ocr_boxes, clicked_index);

    if line.is_empty() {
        return clicked.text.trim().to_string();
    }

    println!("Same visual line contains {} OCR boxes", line.len());

    // ------------------------------------------------------------
    // Find position of clicked box INSIDE sorted line.
    //
    // Important:
    //
    // line contains ORIGINAL OCR indexes.
    // Therefore we don't need references/lifetimes at all.
    // ------------------------------------------------------------

    let clicked_position = line.iter().position(|index| *index == clicked_index);

    let Some(clicked_position) = clicked_position else {
        return clicked.text.trim().to_string();
    };

    println!(
        "Clicked box position inside visual line: {}",
        clicked_position
    );

    // ------------------------------------------------------------
    // Expand left.
    // ------------------------------------------------------------

    let mut left = clicked_position;

    while left > 0 {
        let current_index = line[left];
        let previous_index = line[left - 1];

        let current = &ocr_boxes[current_index];
        let previous = &ocr_boxes[previous_index];

        // A sentence-ending punctuation before the clicked word
        // means the previous sentence is finished.
        if ends_sentence(previous.text.trim()) {
            break;
        }

        // Large horizontal gap means a separate visual text block.
        if is_large_horizontal_gap(previous, current) {
            break;
        }

        left -= 1;
    }

    // ------------------------------------------------------------
    // Expand right.
    // ------------------------------------------------------------

    let mut right = clicked_position;

    while right + 1 < line.len() {
        let current_index = line[right];
        let next_index = line[right + 1];

        let current = &ocr_boxes[current_index];
        let next = &ocr_boxes[next_index];

        // Add the next box first.
        right += 1;

        // If this next OCR box ends a sentence,
        // this is where we stop.
        if ends_sentence(next.text.trim()) {
            break;
        }

        // Large visual gap means separate text block.
        if is_large_horizontal_gap(current, next) {
            break;
        }
    }

    // ------------------------------------------------------------
    // Build final context.
    // ------------------------------------------------------------

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

    context
}

// ============================================================================
// SAME VISUAL LINE
// ============================================================================
//
// Returns ORIGINAL OCR indexes.
//
// This is intentional:
//
// Vec<usize>
//
// instead of:
//
// Vec<(usize, &OcrBox)>
//
// Therefore there is no lifetime problem.
// ============================================================================

fn collect_same_visual_line(ocr_boxes: &[OcrBox], clicked_index: usize) -> Vec<usize> {
    if clicked_index >= ocr_boxes.len() {
        return Vec::new();
    }

    let clicked = &ocr_boxes[clicked_index];

    let (_clicked_min_x, clicked_min_y, _clicked_max_x, clicked_max_y) = clicked.bounding_rect();

    let clicked_height = (clicked_max_y - clicked_min_y).max(1.0);

    let clicked_center_y = (clicked_min_y + clicked_max_y) / 2.0;

    // OCR boxes can have slightly different heights.
    //
    // The tolerance is based primarily on clicked text height.
    let tolerance = (clicked_height * 0.65).max(8.0);

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
//
// OCR normally does not return spaces as separate boxes.
//
// Therefore a "space" is represented by the horizontal distance
// between the previous box and the next box.
//
// We use text height as the scale so this works with different
// font sizes / resolutions.
// ============================================================================

fn is_large_horizontal_gap(left: &OcrBox, right: &OcrBox) -> bool {
    let (left_min_x, left_min_y, left_max_x, left_max_y) = left.bounding_rect();

    let (right_min_x, right_min_y, right_max_x, right_max_y) = right.bounding_rect();

    let _ = left_min_x;
    let _ = right_max_x;

    let gap = right_min_x - left_max_x;

    // If boxes overlap, they are definitely not separated
    // by a large text gap.
    if gap <= 0.0 {
        return false;
    }

    let left_height = (left_max_y - left_min_y).abs();

    let right_height = (right_max_y - right_min_y).abs();

    let text_height = left_height.max(right_height).max(1.0);

    // A normal word-space is usually relatively small.
    //
    // A gap around >= 1.5 text-heights is treated as a
    // separate visual block.
    //
    // This number can be tuned later if necessary.
    let threshold = text_height * 1.5;

    gap >= threshold
}

// ============================================================================
// OCR TEXT JOINING
// ============================================================================
//
// OCR boxes sometimes already contain punctuation such as:
//
// "hello"
// "world."
//
// We don't need sophisticated language processing here.
// We simply preserve OCR text and put normal spaces between boxes.
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
//
// Only punctuation defines a sentence boundary.
//
// No English-word blacklist.
// No UI-word blacklist.
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
