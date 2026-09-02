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
                let click_at = Instant::now();

                // Second click closes the overlay.
                if self.visible {
                    self.overlay.hide();
                    self.visible = false;
                    return;
                }

                match self.lookup(x, y, click_at) {
                    Ok((result, _clicked_ocr_bbox)) => {
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
        // =========================================================
        // SCREENSHOT
        // =========================================================

        let captured = capture_screen(click_x, click_y)?;

        // =========================================================
        // OCR
        // =========================================================

        let ocr_started = Instant::now();

        let ocr_boxes = {
            let mut ocr = self
                .ocr
                .lock()
                .map_err(|_| "OCR engine mutex poisoned".to_string())?;

            ocr.recognize(&captured.image)
                .map_err(|error| format!("OCR failed: {error:#}"))?
        };

        println!("OCR заняло {} ms", ocr_started.elapsed().as_millis());

        if ocr_boxes.is_empty() {
            return Err("OCR detected no text".to_string());
        }

        // =========================================================
        // FIND CLICKED OCR BOX
        // =========================================================

        let local_x = captured.click_x as f32;
        let local_y = captured.click_y as f32;

        let clicked_index = find_clicked_box(&ocr_boxes, local_x, local_y, click_x, click_y)?;

        let clicked_ocr = &ocr_boxes[clicked_index];

        let clicked_word = clicked_ocr.text.trim().to_string();

        if clicked_word.is_empty() {
            return Err("OCR found an empty word under cursor".to_string());
        }

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

        // =========================================================
        // BUILD FULL SENTENCE CONTEXT
        // =========================================================

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        if context.trim().is_empty() {
            return Err("Failed to build OCR context".to_string());
        }

        // =========================================================
        // AI PROVIDER
        // =========================================================

        let result = self.provider.lookup(&context, &clicked_word)?;

        println!("Всего времени {} сек", click_at.elapsed().as_secs());

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

// Минимальное отношение высоты текста кандидата к референсной высоте
// фразы, при котором они считаются набранными одним шрифтом.
//
// Порог намеренно жёсткий, по правилу пользователя: «кликнули в слово
// 18px, следом идёт 20px — предложение закончилось» (18/20 = 0.9 —
// уже граница). Референс — медиана высот уже собранных боксов фразы,
// поэтому останавливается и одиночная смена кегля, и плавная
// деградация (18 → 20 → 22).
//
// Крупные субтитры (~55px) рядом с мелким UI (~25-35px) дают отношение
// ~0.5-0.64: такие блоки по-прежнему не склеиваются — ни внутри одной
// визуальной строки, ни при переходе между строками.
//
// Известный компромисс: OCR-бокс слова с выносными элементами (g/y/p)
// выше бокса слова без них того же кегля (отношение ~0.8-0.9), поэтому
// на реальных скриншотах возможны преждевременные стопы. Если начнут
// мешать — ослабить одной константой (например, до 0.85).
const MIN_TEXT_HEIGHT_RATIO: f32 = 0.92;

// Во сколько раз зазор между боксами должен превышать «обычный» пробел,
// чтобы считаться границей предложения / переходом в другой блок.
//
// Пример пользователя: обычные пробелы текста 2px, встретили 5px — стоп
// (5 >= 2 × 2.5). Работает в паре с абсолютным предохранителем
// «высота шрифта × SENTENCE_GAP_MULTIPLIER» (см. ADAPTIVE SPACE WIDTH).
const SENTENCE_GAP_MULTIPLIER: f32 = 2.5;

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

    // =========================================================
    // BUILD VISUAL LINES
    // =========================================================

    let lines = collect_visual_lines(ocr_boxes);

    // DEBUG: геометрия строк — видно, как OCR-боксы склеились в строки
    // и почему предложение могло уехать не туда.
    println!("[sentence] clicked box: \"{}\"", clicked.text.trim());

    for (line_index, line) in lines.iter().enumerate() {
        let text = line
            .iter()
            .map(|&index| ocr_boxes[index].text.trim())
            .collect::<Vec<_>>()
            .join(" | ");

        println!("[sentence] line {line_index}: {text}");
    }

    if lines.is_empty() {
        return clicked.text.trim().to_string();
    }

    // =========================================================
    // FIND CLICKED LINE
    // =========================================================

    let clicked_line_index = lines.iter().position(|line| line.contains(&clicked_index));

    let Some(clicked_line_index) = clicked_line_index else {
        return clicked.text.trim().to_string();
    };

    let clicked_position = lines[clicked_line_index]
        .iter()
        .position(|index| *index == clicked_index);

    let Some(clicked_position) = clicked_position else {
        return clicked.text.trim().to_string();
    };

    // =========================================================
    // SEED ADAPTIVE SPACE STATS
    // =========================================================

    // Адаптивная статистика пробелов: обычные пробелы одного текста
    // однородны, поэтому «широкий» зазор — сигнал конца предложения
    // или перехода в другой блок.
    let mut gap_stats = GapStats::new();

    seed_gap_stats_from_line(
        ocr_boxes,
        &lines[clicked_line_index],
        clicked_index,
        &mut gap_stats,
    );

    // Референсная высота фразы: начинаем с кликнутого слова и
    // дополняем высотами принятых боксов при обходе в обе стороны.
    let mut phrase_heights: Vec<f32> = vec![box_text_height(clicked)];

    // =========================================================
    // FIND SENTENCE START
    // =========================================================

    let mut start_line = clicked_line_index;
    let mut start_position = clicked_position;

    let mut traversed_lines = 0usize;
    let mut traversed_boxes = 0usize;

    'find_start: loop {
        if traversed_boxes >= MAX_SENTENCE_BOXES {
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

            // Широкий зазор внутри строки — конец предложения или
            // отдельный блок: адаптивный порог (сравнение с обычными
            // пробелами фразы) плюс абсолютный предохранитель.
            if is_sentence_gap_boundary(previous, current, &gap_stats) {
                break;
            }

            // Высота кандидата сверяется с референсом всей фразы,
            // а не только с соседним боксом — иначе плавная смена
            // кегля «утекает» по UI-блокам (см. MIN_TEXT_HEIGHT_RATIO).
            if !text_heights_similar(
                phrase_reference_height(&phrase_heights),
                box_text_height(previous),
            ) {
                break;
            }

            // Бокс принят — пополняем статистику фразы.
            phrase_heights.push(box_text_height(previous));

            gap_stats.push(horizontal_gap(previous, current));

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
            break;
        }

        let previous_line_index = start_line - 1;

        let previous_line = &lines[previous_line_index];

        let current_line = &lines[start_line];

        if !lines_can_be_continuous(ocr_boxes, previous_line, current_line) {
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

            if is_sentence_gap_boundary(current, next, &gap_stats) {
                break;
            }

            // Высота кандидата сверяется с референсом всей фразы
            // (см. комментарий в find_start).
            let next_height = box_text_height(next);

            if !text_heights_similar(phrase_reference_height(&phrase_heights), next_height) {
                break;
            }

            // Бокс принят — пополняем статистику фразы.
            phrase_heights.push(next_height);

            gap_stats.push(horizontal_gap(current, next));

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
            break;
        }

        let next_line_index = end_line + 1;

        let current_line = &lines[end_line];

        let next_line = &lines[next_line_index];

        if !lines_can_be_continuous(ocr_boxes, current_line, next_line) {
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

    println!("[sentence] extracted: \"{context}\"");

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
// TEXT HEIGHT SIMILARITY
// ============================================================================
//
// Единственный надёжный разделитель «своего» текста от чужого UI:
// высота шрифта. Перенесённое предложение набрано тем же кеглем,
// а кнопки/чипы/строки ввода рядом с субтитрами — заметно мельче.

fn box_text_height(ocr_box: &OcrBox) -> f32 {
    let (_, min_y, _, max_y) = ocr_box.bounding_rect();

    (max_y - min_y).abs()
}

fn text_heights_similar(a: f32, b: f32) -> bool {
    let a = a.max(1.0);

    let b = b.max(1.0);

    a.min(b) / a.max(b) >= MIN_TEXT_HEIGHT_RATIO
}

fn similar_text_height(a: &OcrBox, b: &OcrBox) -> bool {
    text_heights_similar(box_text_height(a), box_text_height(b))
}

/// «Доминирующая» высота строки — высота самого крупного бокса.
///
/// Визуальная строка может склеивать боксы разных кеглей (мелкий UI
/// рядом с крупными субтитрами), поэтому сравниваем строки по максимуму:
/// именно он определяет, к какому блоку относится текст.
fn line_dominant_height(ocr_boxes: &[OcrBox], line: &[usize]) -> f32 {
    line.iter()
        .map(|&index| box_text_height(&ocr_boxes[index]))
        .fold(1.0_f32, f32::max)
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
// 1. одинаковую высоту шрифта строк;
// 2. вертикальный gap;
// 3. горизонтальную близость строк;
// 4. отсутствие огромного скачка по X.
//

fn lines_can_be_continuous(ocr_boxes: &[OcrBox], previous: &[usize], current: &[usize]) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    // Разная высота шрифта — разные визуальные блоки (крупные субтитры
    // и мелкий UI над/под ними, заголовок и абзац). Такое соседство
    // плотно по вертикали, поэтому gap-проверка ниже его не ловит.
    if !text_heights_similar(
        line_dominant_height(ocr_boxes, previous),
        line_dominant_height(ocr_boxes, current),
    ) {
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
// ADAPTIVE SPACE WIDTH
// ============================================================================
//
// Обычные пробелы одного текстового блока довольно однородны, поэтому
// «широкий» зазор между соседними боксами — это конец предложения,
// переход в другой блок или колонка. Два критерия (достаточно любого):
//
// 1. Адаптивный: gap >= медиана обычных пробелов фразы × MULTIPLIER.
//    Пример пользователя: обычные пробелы 2px, встретили 5px — стоп.
// 2. Абсолютный: gap >= высота шрифта × MULTIPLIER — предохранитель от
//    вырожденной статистики (фраза пока состоит из одного слова, сидов
//    нет).
//
// Зазоры при переносе строки здесь не участвуют: для них отдельная
// проверка lines_can_be_continuous.

/// Медиана набора значений. Устойчива к выбросам: одиночный широкий
/// зазор (justified-выравнивание, шум OCR) её не тянет.
fn median_of(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }

    let mut sorted = values.to_vec();

    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let middle = sorted.len() / 2;

    if sorted.len() % 2 == 0 {
        Some((sorted[middle - 1] + sorted[middle]) / 2.0)
    } else {
        Some(sorted[middle])
    }
}

/// Накопленная статистика горизонтальных зазоров текущей фразы.
struct GapStats {
    gaps: Vec<f32>,
}

impl GapStats {
    fn new() -> Self {
        Self { gaps: Vec::new() }
    }

    fn push(&mut self, gap: f32) {
        if gap.is_finite() && gap > 0.0 {
            self.gaps.push(gap);
        }
    }

    fn median(&self) -> Option<f32> {
        median_of(&self.gaps)
    }
}

/// Горизонтальный зазор между правым краем `left` и левым краем `right`.
fn horizontal_gap(left: &OcrBox, right: &OcrBox) -> f32 {
    let (_left_min_x, _left_min_y, left_max_x, _left_max_y) = left.bounding_rect();

    let (right_min_x, _right_min_y, _right_max_x, _right_max_y) = right.bounding_rect();

    right_min_x - left_max_x
}

/// Сидирование статистики пробелов: зазоры кликнутой визуальной строки
/// между боксами одного кегля.
///
/// Нужно, чтобы широкая граница СРАЗУ рядом с кликнутым словом тоже
/// распознавалась: без сидов на первом шаге адаптивному критерию не с
/// чем сравнивать. Пары с участием кликнутого бокса не учитываем — их
/// зазор сам является кандидатом на границу предложения и медиану
/// испортит.
fn seed_gap_stats_from_line(
    ocr_boxes: &[OcrBox],
    line: &[usize],
    clicked_index: usize,
    gap_stats: &mut GapStats,
) {
    for pair in line.windows(2) {
        let left_index = pair[0];

        let right_index = pair[1];

        if left_index == clicked_index || right_index == clicked_index {
            continue;
        }

        let left = &ocr_boxes[left_index];

        let right = &ocr_boxes[right_index];

        // Обычные пробелы считаем только между словами одного кегля:
        // зазор между субтитрами и мелким UI — не пробел текста.
        if !similar_text_height(left, right) {
            continue;
        }

        gap_stats.push(horizontal_gap(left, right));
    }
}

/// Граница предложения по ширине зазора (см. ADAPTIVE SPACE WIDTH).
fn is_sentence_gap_boundary(left: &OcrBox, right: &OcrBox, gap_stats: &GapStats) -> bool {
    let gap = horizontal_gap(left, right);

    if gap <= 0.0 {
        return false;
    }

    let (_, left_min_y, _, left_max_y) = left.bounding_rect();

    let (_, right_min_y, _, right_max_y) = right.bounding_rect();

    let text_height = (left_max_y - left_min_y)
        .abs()
        .max((right_max_y - right_min_y).abs())
        .max(1.0);

    let absolute_threshold = text_height * SENTENCE_GAP_MULTIPLIER;

    let median = gap_stats.median();

    let adaptive_threshold = median.map(|value| value * SENTENCE_GAP_MULTIPLIER);

    let stop = match adaptive_threshold {
        Some(threshold) => gap >= threshold || gap >= absolute_threshold,
        None => gap >= absolute_threshold,
    };

    println!(
        "[sentence] gap={gap:.1}px median={:.1}px adaptive={:.1}px absolute={absolute_threshold:.1}px -> {}",
        median.unwrap_or(0.0),
        adaptive_threshold.unwrap_or(0.0),
        if stop { "STOP" } else { "go" }
    );

    stop
}

/// Референсная высота фразы — медиана высот уже собранных боксов.
fn phrase_reference_height(phrase_heights: &[f32]) -> f32 {
    median_of(phrase_heights).unwrap_or(1.0)
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
//     .  ?  !  ;  :  …
//
// и закрывающие кавычки / скобки:
//
//     ."  .)  .]  ?"  ;»
//
// Например:
//
//     "This is a sentence."
//     Wait; then: go.
//
// корректно определяются как конец предложения.
//

fn ends_sentence(text: &str) -> bool {
    let text = text.trim();

    if text.is_empty() {
        return false;
    }

    let without_closing =
        text.trim_end_matches(|c: char| matches!(c, '"' | '\'' | ')' | ']' | '}' | '»'));

    without_closing.ends_with('.')
        || without_closing.ends_with('?')
        || without_closing.ends_with('!')
        || without_closing.ends_with(';')
        || without_closing.ends_with(':')
        || without_closing.ends_with('…')
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

// ============================================================================
// HIT-TEST
// ============================================================================

/// Расстояние от точки до прямоугольника (0, если точка внутри).
fn distance_to_rect(x: f32, y: f32, rect: (f32, f32, f32, f32)) -> f32 {
    let (min_x, min_y, max_x, max_y) = rect;

    let dx = if x < min_x {
        min_x - x
    } else if x > max_x {
        x - max_x
    } else {
        0.0
    };

    let dy = if y < min_y {
        min_y - y
    } else if y > max_y {
        y - max_y
    } else {
        0.0
    };

    (dx * dx + dy * dy).sqrt()
}

/// Поиск OCR-бокса, в который попал клик.
///
/// 1. Точное попадание точки в полигон бокса; при нескольких попаданиях
///    берётся наименьший по площади (самое конкретное слово).
/// 2. Fallback: полигоны OCR обрезаны вплотную к глифам, и клик часто
///    попадает в зазор между буквами/словами или на пиксель выше/ниже
///    строки. Тогда берём ближайший бокс, если клик не дальше допуска
///    (60% высоты бокса, но не меньше 8 px) — в пределах строки это
///    прощает промах, а текст из чужого блока интерфейса не зацепит.
/// 3. Полный промах — ошибка; в консоль печатается клик и все боксы,
///    чтобы по логу видеть, что именно распознал OCR и где был клик.
fn find_clicked_box(
    ocr_boxes: &[OcrBox],
    local_x: f32,
    local_y: f32,
    global_x: i32,
    global_y: i32,
) -> Result<usize, String> {
    let exact = ocr_boxes
        .iter()
        .enumerate()
        .filter(|(_, ocr_box)| ocr_box.contains_point(local_x, local_y))
        .min_by(|(_, a), (_, b)| {
            bbox_area(a)
                .partial_cmp(&bbox_area(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index);

    if let Some(index) = exact {
        return Ok(index);
    }

    let mut nearest: Option<(usize, f32)> = None;

    for (index, ocr_box) in ocr_boxes.iter().enumerate() {
        let rect = ocr_box.bounding_rect();

        let height = (rect.3 - rect.1).max(0.0);

        let tolerance = (height * 0.6).max(8.0);

        let distance = distance_to_rect(local_x, local_y, rect);

        if distance <= tolerance && nearest.map_or(true, |(_, best)| distance < best) {
            nearest = Some((index, distance));
        }
    }

    if let Some((index, distance)) = nearest {
        println!(
            "Lookup hit-test: точный промах, взят ближайший бокс '{}' ({distance:.1} px)",
            ocr_boxes[index].text.trim()
        );

        return Ok(index);
    }

    println!(
        "Lookup miss: клик экрана ({global_x}, {global_y}) -> локальные кропа ({local_x:.0}, {local_y:.0}); OCR боксы ({}):",
        ocr_boxes.len()
    );

    for ocr_box in ocr_boxes {
        let (min_x, min_y, max_x, max_y) = ocr_box.bounding_rect();

        println!(
            "  '{}' @ [{min_x:.0},{min_y:.0} .. {max_x:.0},{max_y:.0}]",
            ocr_box.text.trim()
        );
    }

    Err("No OCR text region found under cursor".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app_core::ocr::types::OcrPoint;

    // ------------------------------------------------------------------
    // find_clicked_box: попадание клика в OCR-бокс.
    // ------------------------------------------------------------------

    #[test]
    fn hit_test_exact_containment_picks_smallest_box() {
        // Клик внутри двух вложенных боксов: побеждает меньший
        // (более конкретное слово) — прежнее поведение сохранено.
        let boxes = vec![
            box_at(0.0, 0.0, 200.0, 100.0, "huge"),
            box_at(10.0, 10.0, 50.0, 30.0, "small"),
        ];

        let index = find_clicked_box(&boxes, 30.0, 20.0, 1000, 1000).expect("клик внутри бокса");

        assert_eq!(boxes[index].text, "small");
    }

    #[test]
    fn hit_test_falls_back_to_nearest_box_in_gap() {
        // Клик в зазор между словами: полигоны OCR обрезаны вплотную к
        // глифам, точного попадания нет — берётся ближайший бокс строки.
        let boxes = vec![
            box_at(0.0, 0.0, 40.0, 20.0, "Hello"),
            box_at(46.0, 0.0, 90.0, 20.0, "World"),
        ];

        let index =
            find_clicked_box(&boxes, 43.0, 10.0, 1000, 1000).expect("зазор в пределах допуска");

        assert_eq!(boxes[index].text, "Hello");
    }

    #[test]
    fn hit_test_falls_back_to_click_above_line() {
        // Клик на два пикселя выше строки — тоже прощается.
        let boxes = vec![box_at(10.0, 10.0, 50.0, 30.0, "word")];

        let index = find_clicked_box(&boxes, 30.0, 8.0, 1000, 1000)
            .expect("клик над строкой в пределах допуска");

        assert_eq!(boxes[index].text, "word");
    }

    #[test]
    fn hit_test_rejects_click_far_from_any_text() {
        // Клик по пустому месту вдали от текста: ошибка, как и раньше —
        // допуск не должен цеплять текст из чужого блока интерфейса.
        let boxes = vec![box_at(10.0, 10.0, 50.0, 30.0, "word")];

        assert!(find_clicked_box(&boxes, 300.0, 300.0, 1000, 1000).is_err());
    }

    fn box_at(min_x: f32, min_y: f32, max_x: f32, max_y: f32, text: &str) -> OcrBox {
        OcrBox {
            points: [
                OcrPoint { x: min_x, y: min_y },
                OcrPoint { x: max_x, y: min_y },
                OcrPoint { x: max_x, y: max_y },
                OcrPoint { x: min_x, y: max_y },
            ],
            confidence: 0.9,
            text: text.to_string(),
        }
    }

    /// Модель экрана из реального промаха: крупные субтитры (~56px)
    /// поверх плотного мелкого UI (~32px). Клик по слову «remember».
    ///
    /// Строка ввода («cd real-es») и субтитры лежат на одном визуальном
    /// ряду, а UI выше/ниже отделён маленькими gap'ами, которые проходят
    /// проверку MAX_LINE_GAP_MULTIPLIER. Раньше это склеивалось в одну
    /// «предложение» вместе с чипом и строкой ввода над субтитрами.
    fn subtitle_over_ui_boxes() -> Vec<OcrBox> {
        vec![
            // Терминал над субтитрами.
            box_at(0.0, 0.0, 400.0, 36.0, "Change directory into the new project."),
            // Мелкий UI между терминалом и субтитрами.
            box_at(40.0, 44.0, 300.0, 76.0, "~/Documents/Builds"),
            // Строка ввода и субтитры на одном визуальном ряду.
            box_at(0.0, 84.0, 90.0, 116.0, "cd"),
            box_at(100.0, 84.0, 260.0, 116.0, "real-es"),
            box_at(270.0, 80.0, 430.0, 136.0, "available"),
            box_at(440.0, 80.0, 540.0, 136.0, "right"),
            box_at(550.0, 80.0, 600.0, 136.0, "so"),
            box_at(610.0, 80.0, 760.0, 136.0, "remember"),
            box_at(770.0, 80.0, 840.0, 136.0, "you"),
            box_at(850.0, 80.0, 920.0, 136.0, "can"),
            // Вторая строка субтитров.
            box_at(270.0, 140.0, 450.0, 196.0, "actually"),
            box_at(460.0, 140.0, 540.0, 196.0, "just"),
            box_at(550.0, 140.0, 610.0, 196.0, "go"),
            // Мелкий UI под субтитрами.
            box_at(40.0, 210.0, 100.0, 240.0, "LIVE"),
        ]
    }

    #[test]
    fn sentence_stays_inside_subtitle_font() {
        let ocr_boxes = subtitle_over_ui_boxes();

        let clicked_index = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "remember")
            .unwrap();

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        assert_eq!(
            context, "available right so remember you can actually just go",
            "предложение не должно захватывать мелкий UI выше и ниже субтитров"
        );
    }

    #[test]
    fn wrapped_paragraph_of_same_font_still_joins() {
        let ocr_boxes = vec![
            box_at(40.0, 0.0, 300.0, 32.0, "The readiness and zeal with which"),
            box_at(40.0, 44.0, 340.0, 76.0, "these builders set about their work."),
        ];

        let clicked_index = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text.starts_with("these"))
            .unwrap();

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        assert_eq!(
            context,
            "The readiness and zeal with which these builders set about their work."
        );
    }

    #[test]
    fn similar_height_accepts_wrapped_lines_rejects_ui() {
        let subtitle_line = box_at(0.0, 80.0, 100.0, 136.0, "big");

        let subtitle_next_line = box_at(0.0, 140.0, 100.0, 196.0, "big2");

        let ui_chip = box_at(0.0, 44.0, 50.0, 76.0, "small");

        assert!(similar_text_height(&subtitle_line, &subtitle_next_line));

        assert!(!similar_text_height(&subtitle_line, &ui_chip));
    }

    /// Пример пользователя: клик в слово 18px, рядом слово 20px —
    /// смена кегля останавливает обход (18/20 = 0.9 < 0.92).
    /// Зазоры между словами везде одинаковые, чтобы срабатывал
    /// именно критерий высоты шрифта, а не пробелов.
    #[test]
    fn different_font_size_stops_run() {
        let ocr_boxes = vec![
            box_at(0.0, 0.0, 80.0, 18.0, "first"),
            box_at(90.0, 0.0, 200.0, 20.0, "second"),
            box_at(210.0, 0.0, 320.0, 20.0, "third"),
        ];

        let context = extract_sentence_context(&ocr_boxes, 0);

        assert_eq!(
            context, "first",
            "20px справа после кликнутого 18px — смена кегля, вправо не идём"
        );

        let context = extract_sentence_context(&ocr_boxes, 2);

        assert_eq!(
            context, "second third",
            "18px слева от 20px — смена кегля, влево не идём"
        );
    }

    /// Обычные пробелы 4px, затем зазор 12px — граница предложения
    /// по адаптивному критерию (12 >= 4 × 2.5).
    #[test]
    fn wide_gap_stops_sentence() {
        let ocr_boxes = vec![
            box_at(0.0, 0.0, 60.0, 32.0, "one"),
            box_at(64.0, 0.0, 124.0, 32.0, "two"),
            box_at(128.0, 0.0, 188.0, 32.0, "three"),
            box_at(192.0, 0.0, 252.0, 32.0, "four"),
            box_at(264.0, 0.0, 324.0, 32.0, "five"),
        ];

        let clicked_index = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "three")
            .unwrap();

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        assert_eq!(
            context, "one two three four",
            "зазор 12px при обычных 4px — граница предложения"
        );
    }

    /// Широкий зазор СРАЗУ слева от кликнутого слова: без сидирования
    /// статистики пробелов первый шаг не с чем сравнивать.
    #[test]
    fn wide_gap_next_to_clicked_stops() {
        let ocr_boxes = vec![
            box_at(0.0, 0.0, 60.0, 32.0, "one"),
            box_at(64.0, 0.0, 124.0, 32.0, "two"),
            box_at(136.0, 0.0, 196.0, 32.0, "three"),
        ];

        let clicked_index = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "three")
            .unwrap();

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        assert_eq!(
            context, "three",
            "зазор 12px при сидированных пробелах 4px — стоп на первом шаге"
        );
    }

    /// Умеренный разброс пробелов (4px → 6px, отношение 1.5) —
    /// не граница: justified-текст не должен рваться.
    #[test]
    fn moderate_gap_continues() {
        let ocr_boxes = vec![
            box_at(0.0, 0.0, 60.0, 32.0, "one"),
            box_at(64.0, 0.0, 124.0, 32.0, "two"),
            box_at(128.0, 0.0, 188.0, 32.0, "three"),
            box_at(192.0, 0.0, 252.0, 32.0, "four"),
            box_at(256.0, 0.0, 316.0, 32.0, "five"),
            box_at(322.0, 0.0, 382.0, 32.0, "six"),
        ];

        let clicked_index = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "three")
            .unwrap();

        let context = extract_sentence_context(&ocr_boxes, clicked_index);

        assert_eq!(
            context, "one two three four five six",
            "разброс пробелов 4→6px предложение не рвёт"
        );
    }

    /// `;` и `:` — тоже конец предложения (клик в слово, у которого
    /// слева/справа такой знак).
    #[test]
    fn semicolon_and_colon_end_sentence() {
        let ocr_boxes = vec![
            box_at(0.0, 0.0, 60.0, 32.0, "Wait;"),
            box_at(70.0, 0.0, 130.0, 32.0, "then:"),
            box_at(140.0, 0.0, 190.0, 32.0, "go"),
        ];

        let clicked_then = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "then:")
            .unwrap();

        assert_eq!(
            extract_sentence_context(&ocr_boxes, clicked_then),
            "then:",
            "клик в слово с ':' — оно включается, обход дальше не идёт"
        );

        let clicked_wait = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "Wait;")
            .unwrap();

        assert_eq!(
            extract_sentence_context(&ocr_boxes, clicked_wait),
            "Wait;",
            "клик в слово с ';' — оно включается, обход дальше не идёт"
        );

        let clicked_go = ocr_boxes
            .iter()
            .position(|ocr_box| ocr_box.text == "go")
            .unwrap();

        assert_eq!(
            extract_sentence_context(&ocr_boxes, clicked_go),
            "go",
            "«then:» слева — граница предложения"
        );
    }
}
