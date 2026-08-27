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
                        println!("meaning: {}", result.meaning);
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
        // BUILD FULL SENTENCE CONTEXT
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
// Основная идея:
//
// Мы больше НЕ используем:
//     LEFT_CONTEXT_WORDS
//     RIGHT_CONTEXT_WORDS
//     MAX_CONTEXT_WIDTH_MULTIPLIER
//
// Потому что они искусственно обрезали предложение.
//
// Вместо этого:
//
// 1. Собираем OCR boxes в визуальные строки.
// 2. Находим строку, в которой находится clicked word.
// 3. Идём назад до начала предложения.
// 4. Идём вперёд до конца предложения.
// 5. Если предложение переносится на следующую строку,
//    продолжаем туда.
// 6. При этом не пересекаем явно отделённые UI-блоки.
//
// Таким образом:
//
//     "The readiness and zeal with these builders ..."
//                    ^
//                 CLICK
//
// даст целое предложение от начала до ".".
//

// Абсолютный предохранитель от патологического OCR.
// Нормальное предложение сюда никогда не должно упираться.
//
// 120 OCR boxes — это очень много для одного предложения,
// но оставляет достаточно места для длинных книжных предложений.
const MAX_SENTENCE_BOXES: usize = 120;

// Максимальное количество визуальных строк,
// через которые разрешаем переходить при поиске предложения.
const MAX_SENTENCE_LINES: usize = 12;

// Насколько далеко по вертикали соседняя строка может находиться
// относительно высоты текста.
//
// Например:
//
// line 1
//
// line 2
//
// нормальный межстрочный интервал проходит.
//
// Огромный вертикальный скачок — вероятно другой UI block.
const MAX_LINE_GAP_MULTIPLIER: f32 = 2.2;

// ============================================================================
// MAIN SENTENCE EXTRACTION
// ============================================================================

fn extract_sentence_context(ocr_boxes: &[OcrBox], clicked_index: usize) -> String {
    if ocr_boxes.is_empty() {
        return String::new();
    }

    if clicked_index >= ocr_boxes.len() {
        return String::new();
    }

    let clicked = &ocr_boxes[clicked_index];

    println!(
        "Building full sentence context around '{}'",
        clicked.text.trim()
    );

    // =========================================================
    // BUILD VISUAL LINES
    // =========================================================

    let lines = collect_visual_lines(ocr_boxes);

    if lines.is_empty() {
        return clicked.text.trim().to_string();
    }

    println!("OCR contains {} visual lines", lines.len());

    // =========================================================
    // FIND CLICKED LINE
    // =========================================================

    let clicked_line_index = lines.iter().position(|line| line.contains(&clicked_index));

    let Some(clicked_line_index) = clicked_line_index else {
        return clicked.text.trim().to_string();
    };

    println!("Clicked word belongs to visual line {}", clicked_line_index);

    let clicked_position = lines[clicked_line_index]
        .iter()
        .position(|index| *index == clicked_index);

    let Some(clicked_position) = clicked_position else {
        return clicked.text.trim().to_string();
    };

    println!("Clicked word position inside line: {}", clicked_position);

    // =========================================================
    // FIND SENTENCE START
    // =========================================================

    let mut start_line = clicked_line_index;
    let mut start_position = clicked_position;

    let mut traversed_lines = 0usize;
    let mut traversed_boxes = 0usize;

    'find_start: loop {
        if traversed_boxes >= MAX_SENTENCE_BOXES {
            println!("Sentence start search stopped at MAX_SENTENCE_BOXES");
            break;
        }

        if start_position > 0 {
            let previous_index = lines[start_line][start_position - 1];

            let current_index = lines[start_line][start_position];

            let previous = &ocr_boxes[previous_index];

            let current = &ocr_boxes[current_index];

            // Если предыдущий box заканчивает предложение,
            // начало нашего предложения — текущий box.
            if ends_sentence(previous.text.trim()) {
                break;
            }

            // Большой gap внутри строки может означать
            // отдельный UI block.
            if is_large_horizontal_gap(previous, current) {
                println!("Sentence start blocked by large horizontal gap");
                break;
            }

            start_position -= 1;
            traversed_boxes += 1;

            continue;
        }

        // Мы дошли до начала визуальной строки.
        //
        // Теперь пытаемся перейти на предыдущую строку.
        if start_line == 0 {
            break;
        }

        if traversed_lines >= MAX_SENTENCE_LINES {
            println!("Sentence start search stopped at MAX_SENTENCE_LINES");
            break;
        }

        let previous_line_index = start_line - 1;

        let previous_line = &lines[previous_line_index];

        let current_line = &lines[start_line];

        if !lines_can_be_continuous(ocr_boxes, previous_line, current_line) {
            println!("Previous visual line is not continuous with current line");
            break;
        }

        let previous_last_position = previous_line.len() - 1;

        let previous_last_index = previous_line[previous_last_position];

        let previous_last = &ocr_boxes[previous_last_index];

        // Если предыдущая строка закончилась точкой,
        // значит наше предложение начинается здесь.
        if ends_sentence(previous_last.text.trim()) {
            break;
        }

        // Переходим в конец предыдущей строки.
        start_line = previous_line_index;
        start_position = previous_last_position;

        traversed_lines += 1;
        traversed_boxes += 1;
    }

    // =========================================================
    // FIND SENTENCE END
    // =========================================================

    let mut end_line = clicked_line_index;
    let mut end_position = clicked_position;

    traversed_lines = 0;
    traversed_boxes = 0;

    'find_end: loop {
        if traversed_boxes >= MAX_SENTENCE_BOXES {
            println!("Sentence end search stopped at MAX_SENTENCE_BOXES");
            break;
        }

        let current_index = lines[end_line][end_position];

        let current = &ocr_boxes[current_index];

        // Если текущий box уже заканчивает предложение,
        // включаем его и останавливаемся.
        if ends_sentence(current.text.trim()) {
            break 'find_end;
        }

        // =====================================================
        // NEXT BOX IN SAME LINE
        // =====================================================

        if end_position + 1 < lines[end_line].len() {
            let next_index = lines[end_line][end_position + 1];

            let next = &ocr_boxes[next_index];

            if is_large_horizontal_gap(current, next) {
                println!("Sentence end blocked by large horizontal gap");
                break;
            }

            end_position += 1;
            traversed_boxes += 1;

            continue;
        }

        // =====================================================
        // NEXT VISUAL LINE
        // =====================================================

        if end_line + 1 >= lines.len() {
            break;
        }

        if traversed_lines >= MAX_SENTENCE_LINES {
            println!("Sentence end search stopped at MAX_SENTENCE_LINES");
            break;
        }

        let next_line_index = end_line + 1;

        let current_line = &lines[end_line];

        let next_line = &lines[next_line_index];

        if !lines_can_be_continuous(ocr_boxes, current_line, next_line) {
            println!("Next visual line is not continuous with current line");
            break;
        }

        end_line = next_line_index;
        end_position = 0;

        traversed_lines += 1;
        traversed_boxes += 1;
    }

    // =========================================================
    // BUILD FINAL TEXT
    // =========================================================

    let mut parts: Vec<&str> = Vec::new();

    let mut line_index = start_line;

    while line_index <= end_line {
        let line = &lines[line_index];

        let from = if line_index == start_line {
            start_position
        } else {
            0
        };

        let to = if line_index == end_line {
            end_position
        } else {
            line.len().saturating_sub(1)
        };

        if from <= to {
            for position in from..=to {
                let index = line[position];

                let text = ocr_boxes[index].text.trim();

                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }

        if line_index == end_line {
            break;
        }

        line_index += 1;
    }

    let context = join_ocr_text(&parts);

    println!("Sentence range: lines {}..={}", start_line, end_line);

    println!("Sentence contains {} OCR boxes", parts.len());

    println!("Sentence start: '{}'", parts.first().copied().unwrap_or(""));

    println!("Sentence end: '{}'", parts.last().copied().unwrap_or(""));

    println!("FULL SENTENCE CONTEXT: '{}'", context);

    context
}

// ============================================================================
// VISUAL LINES
// ============================================================================
//
// OCR engine отдаёт word-level boxes, поэтому здесь восстанавливаем
// строки по вертикальной позиции.
//
// Важно:
//
// НЕ используем overlap bbox.
//
// Сравниваем именно центры по Y.
//
// Это предотвращает смешивание соседних строк.
//

fn collect_visual_lines(ocr_boxes: &[OcrBox]) -> Vec<Vec<usize>> {
    if ocr_boxes.is_empty() {
        return Vec::new();
    }

    // Сначала сортируем все boxes сверху вниз.
    let mut indexes: Vec<usize> = (0..ocr_boxes.len()).collect();

    indexes.sort_by(|a, b| {
        let (_, a_min_y, _, a_max_y) = ocr_boxes[*a].bounding_rect();

        let (_, b_min_y, _, b_max_y) = ocr_boxes[*b].bounding_rect();

        let a_center_y = (a_min_y + a_max_y) / 2.0;

        let b_center_y = (b_min_y + b_max_y) / 2.0;

        a_center_y
            .partial_cmp(&b_center_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut lines: Vec<Vec<usize>> = Vec::new();

    for index in indexes {
        let (_, min_y, _, max_y) = ocr_boxes[index].bounding_rect();

        let center_y = (min_y + max_y) / 2.0;

        let height = (max_y - min_y).abs().max(1.0);

        let mut added_to_line = false;

        // Ищем ближайшую существующую строку.
        //
        // Идём с конца, потому что boxes уже отсортированы
        // сверху вниз.
        for line in lines.iter_mut().rev() {
            if line.is_empty() {
                continue;
            }

            let reference_index = line[line.len() - 1];

            let (_, ref_min_y, _, ref_max_y) = ocr_boxes[reference_index].bounding_rect();

            let ref_center_y = (ref_min_y + ref_max_y) / 2.0;

            let ref_height = (ref_max_y - ref_min_y).abs().max(1.0);

            let tolerance = (height.max(ref_height) * 0.45).max(6.0);

            if (center_y - ref_center_y).abs() <= tolerance {
                line.push(index);
                added_to_line = true;
                break;
            }

            // Если уже ушли достаточно далеко по Y,
            // старые строки проверять нет смысла.
            if center_y > ref_center_y + height.max(ref_height) * 1.5 {
                break;
            }
        }

        if !added_to_line {
            lines.push(vec![index]);
        }
    }

    // В каждой строке сортируем слева направо.
    for line in &mut lines {
        line.sort_by(|a, b| {
            let ax = ocr_boxes[*a].bounding_rect().0;

            let bx = ocr_boxes[*b].bounding_rect().0;

            ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Дополнительная сортировка строк по Y.
    lines.sort_by(|a, b| {
        let a_index = a[0];
        let b_index = b[0];

        let (_, a_min_y, _, a_max_y) = ocr_boxes[a_index].bounding_rect();

        let (_, b_min_y, _, b_max_y) = ocr_boxes[b_index].bounding_rect();

        let a_center = (a_min_y + a_max_y) / 2.0;

        let b_center = (b_min_y + b_max_y) / 2.0;

        a_center
            .partial_cmp(&b_center)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    lines
}

// ============================================================================
// CHECK WHETHER TWO VISUAL LINES CAN BELONG TO THE SAME SENTENCE
// ============================================================================
//
// Это важно для текста:
//
//     The readiness and zeal with which these
//     builders set about their work, the exactness
//
// Мы должны разрешить переход между строками.
//
// Но при этом не хотим случайно соединить:
//
//     Main paragraph text...
//
//     [BUTTON] Settings
//
// Поэтому проверяем:
//
// 1. вертикальный gap;
// 2. горизонтальную близость строк;
// 3. отсутствие огромного скачка по X.
//

fn lines_can_be_continuous(ocr_boxes: &[OcrBox], previous: &[usize], current: &[usize]) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    let previous_first = &ocr_boxes[previous[0]];

    let previous_last = &ocr_boxes[previous[previous.len() - 1]];

    let current_first = &ocr_boxes[current[0]];

    let previous_bounds = previous_first.bounding_rect();

    let previous_last_bounds = previous_last.bounding_rect();

    let current_bounds = current_first.bounding_rect();

    let previous_min_y = previous_bounds.1;

    let previous_max_y = previous_bounds.3;

    let current_min_y = current_bounds.1;

    let current_max_y = current_bounds.3;

    let previous_height = (previous_max_y - previous_min_y).abs().max(1.0);

    let current_height = (current_max_y - current_min_y).abs().max(1.0);

    let text_height = previous_height.max(current_height);

    let vertical_gap = current_min_y - previous_max_y;

    // Если строки перекрываются или gap небольшой —
    // это нормальный межстрочный интервал.
    if vertical_gap > text_height * MAX_LINE_GAP_MULTIPLIER {
        return false;
    }

    // =========================================================
    // HORIZONTAL CONTINUITY
    // =========================================================
    //
    // Сравниваем начало следующей строки
    // с началом предыдущей.
    //
    // Для обычного wrapped paragraph:
    //
    //     The readiness and zeal with which
    //     these builders set about their work
    //
    // X примерно одинаковый.
    //
    // Если следующий блок начинается очень далеко —
    // вероятно это другой UI block.

    let previous_min_x = previous_bounds.0;

    let previous_max_x = previous_last_bounds.2;

    let current_min_x = current_bounds.0;

    let current_max_x = current_bounds.2;

    let previous_width = (previous_max_x - previous_min_x).abs().max(1.0);

    let current_width = (current_max_x - current_min_x).abs().max(1.0);

    let horizontal_reference = previous_width.max(current_width).max(20.0);

    let x_difference = (current_min_x - previous_min_x).abs();

    // Обычная строка paragraph может начинаться
    // немного левее/правее из-за OCR.
    //
    // Но огромный сдвиг считаем новым блоком.
    if x_difference > horizontal_reference * 2.0 {
        return false;
    }

    true
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

    // Оставляем довольно мягкое ограничение.
    //
    // Нам важно не резать нормальные пробелы между словами.
    let threshold = text_height * 2.5;

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
//
// Учитываем:
//
//     .
//     ?
//     !
//
// и закрывающие кавычки / скобки:
//
//     ."
//     .)
//     .]
//
// Например:
//
//     "This is a sentence."
//
// корректно определяется как конец предложения.
//

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
