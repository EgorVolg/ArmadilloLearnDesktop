use anyhow::{Context, Result};

use image::RgbImage;
use rapidocr_core::{
    config::{ExecutionProvider, InferenceOptions, PipelineConfig},
    model::PPOCRV5_EN_MOBILE,
    RapidOcr,
};

use crate::app_core::lookup::image::Image;

use super::types::{OcrBox, OcrPoint};

/// Верхняя граница intra-op потоков ONNX Runtime.
///
/// rapidocr-core по умолчанию создаёт сессии с ОДНИМ потоком
/// и выключенным memory arena (см. InferenceOptions::default()
/// в rapidocr-core 0.2.2) — на многоядерном CPU инференс
/// получается в разы медленнее возможного.
///
/// Матрица потоков на реальном кропе 1440x900 (65 строк, batch 2):
/// 14 потоков == 8 потоков (rec ~0.9 с), 6 потоков уже медленнее.
/// 8 потоков дают тот же результат с меньшим нагревом/троттлингом.
const OCR_INFERENCE_THREADS_CAP: usize = 8;

/// rec-батч по умолчанию.
///
/// Ширина входа rec-батча динамическая — 48 x ratio САМОЙ ШИРОКОЙ
/// строки в батче, поэтому крупный батч смешивает строки разной
/// ширины и увеличивает объём паддинга. Матрица на реальном кропе
/// 1440x900 (65 строк терминального текста, 14 потоков), steady-state:
///
///   batch  6 -> total ~2.2 с, rec ~1.6 с (~25 мс/строку)
///   batch  4 -> total ~1.7 с, rec ~1.2 с (~18 мс/строку)
///   batch  3 -> total ~1.5 с, rec ~1.0 с (~15 мс/строку)
///   batch  2 -> total ~1.4 с, rec ~0.9 с (~14 мс/строку)  <- оптимум
///   batch  1 -> катастрофа: 65 последовательных session.run,
///               каждый со своей формой тензора, не завершился
///               за разумное время
///
/// Тюнинг без перекомпиляции: ARMADILLO_OCR_REC_BATCH.
const OCR_REC_BATCH_DEFAULT: usize = 2;

pub struct OcrEngine {
    engine: RapidOcr,
    /// Хеш пикселей последнего распознанного кропа.
    ///
    /// Повторные клики по тому же экрану (типичный сценарий чтения:
    /// смотрим слово за словом без прокрутки) отдают боксы из кэша
    /// мгновенно, пропуская det+rec полностью.
    cache_hash: u64,
    cache_boxes: Vec<OcrBox>,
}

impl OcrEngine {
    pub fn new(model_dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let model_dir = model_dir.into();

        let intra_threads = std::env::var("ARMADILLO_OCR_THREADS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=32).contains(value))
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
                    .min(OCR_INFERENCE_THREADS_CAP)
            });

        let rec_batch_size = std::env::var("ARMADILLO_OCR_REC_BATCH")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=64).contains(value))
            .unwrap_or(OCR_REC_BATCH_DEFAULT);

        // Параметры детекции, подобранные под скриншоты рабочего стола
        // (тёмные темы, стилизованные субтитры), с env-переопределением
        // для экспериментов без перекомпиляции.
        //
        // unclip_ratio: расширение полигонов DB-детектора. Дефолт крейта
        // 1.6 слишком тесный для стилизованных/курсивных шрифтов — рамка
        // срезает крайние буквы («Scisso» вместо «Scissors», «womar»
        // вместо «woman»), и rec получает обрубленное слово. 2.0 оставляет
        // место свисающим элементам глифов.
        //
        // box_thresh: минимальная уверенность кандидата детекции. Дефолт
        // 0.5 выбрасывал тусклые строки субтитров на тёмном фоне целиком
        // («не видит все слова»).
        //
        // text_score: финальный фильтр строк по средней уверенности rec.
        // Дефолт 0.5 добивал слабо распознанные, но реальные строки;
        // мусор ниже отсекается нашим is_acceptable_ocr_text.
        let det_unclip = env_f32("ARMADILLO_OCR_DET_UNCLIP", 2.0, 1.0, 4.0);
        let det_box_thresh = env_f32("ARMADILLO_OCR_DET_BOX_THRESH", 0.4, 0.1, 0.9);
        let rec_text_score = env_f32("ARMADILLO_OCR_TEXT_SCORE", 0.4, 0.1, 0.9);

        // Провайдер исполнения: по умолчанию CPU.
        // DirectML включается только явно: ARMADILLO_OCR_EP=dml
        //
        // Почему DirectML не по умолчанию: крейт хардкодит
        // DirectML::default() (device 0), выбора GPU нет в его API.
        // На Optimus-ноутбуках DXGI-адаптер 0 — как правило встроенная
        // Intel-графика, поэтому DirectML попадает в iGPU: скорость
        // не растёт (shared-память), а точность детектора падает
        // (FP16-расхождения дают пропуски текста под курсором).
        let use_direct_ml = std::env::var("ARMADILLO_OCR_EP")
            .map(|value| {
                value.eq_ignore_ascii_case("dml") || value.eq_ignore_ascii_case("directml")
            })
            .unwrap_or(false);

        let build_config = |provider: ExecutionProvider| {
            let inference = InferenceOptions {
                intra_threads,
                // Модели PP-OCR — одиночные графы без ветвлений,
                // параллельный граф-исполнитель только добавляет
                // накладные расходы, а для DirectML он запрещён совсем.
                inter_threads: 1,
                parallel_execution: false,
                enable_cpu_mem_arena: matches!(provider, ExecutionProvider::Cpu),
                execution_provider: provider,
            };

            let mut config = PPOCRV5_EN_MOBILE.config(model_dir.clone());

            // rec-батчи формируются после сортировки кропов по соотношению
            // сторон (TextRecognizer::recognize_timed), но ширина входа
            // батча динамическая — 48 x ratio самой ШИРОКОЙ строки в батче.
            // Маленький батч держит группы однородными по ширине и почти
            // не платит паддингом; полная матрица измерений — в комментарии
            // к OCR_REC_BATCH_DEFAULT выше.
            if let Some(rec) = config.rec.as_mut() {
                rec.batch_size = rec_batch_size;
            }

            // Детекция и фильтр строк: см. комментарий к env_f32-переменным
            // выше — дефолты крейта настроены на сканы документов, а не на
            // тёмные темы и стилизованные субтитры.
            if let Some(det) = config.det.as_mut() {
                det.unclip_ratio = det_unclip;
                det.box_thresh = det_box_thresh;
            }

            config.text_score = rec_text_score;

            config
                // Скриншоты всегда правильной ориентации: классификатор
                // поворота текстовых строк не нужен и только тратит
                // время на каждый кроп.
                .with_pipeline(PipelineConfig::without_cls())
                .with_inference_options(inference)
        };

        let (engine, provider_label) = if use_direct_ml {
            // Экспериментальный путь: если GPU недоступен — откат на CPU.
            match RapidOcr::new(build_config(ExecutionProvider::DirectMl)) {
                Ok(engine) => (engine, "directml (экспериментально, ARMADILLO_OCR_EP=dml)"),
                Err(dml_error) => {
                    println!("OCR: DirectML init failed ({dml_error:#}), falling back to CPU");

                    let engine = RapidOcr::new(build_config(ExecutionProvider::Cpu))
                        .context("failed to initialize PP-OCRv5 English OCR engine")?;
                    (engine, "cpu (DirectML fallback)")
                }
            }
        } else {
            let engine = RapidOcr::new(build_config(ExecutionProvider::Cpu))
                .context("failed to initialize PP-OCRv5 English OCR engine")?;
            (engine, "cpu")
        };

        println!(
            "OCR inference: intra_threads={intra_threads}, rec_batch={rec_batch_size}, pipeline=det+rec, ep={provider_label}, det: unclip={det_unclip} box_thresh={det_box_thresh}, text_score={rec_text_score}"
        );

        Ok(Self {
            engine,
            cache_hash: 0,
            cache_boxes: Vec::new(),
        })
    }

    /// Runs OCR exactly once over the supplied image.
    ///
    /// PP-OCR gives us line-level bounding boxes, so we derive
    /// approximate word-level boxes from the recognized text.
    pub fn recognize(&mut self, image: &Image) -> Result<Vec<OcrBox>> {
        let rgb = image_to_rgb_image(image)?;

        let hash = hash_rgb_image(&rgb);

        if hash != 0 && hash == self.cache_hash {
            println!("OCR cache hit ({} boxes)", self.cache_boxes.len());

            return Ok(self.cache_boxes.clone());
        }

        let boxes = self.recognize_rgb(rgb)?;

        self.cache_hash = hash;
        self.cache_boxes = boxes.clone();

        Ok(boxes)
    }

    /// Прогрев одним холостым инференсом сразу после старта приложения.
    ///
    /// Первый запуск аллоцирует memory arena и инициализирует
    /// GPU-ресурсы DirectML; без прогрева это легло бы на первый клик.
    /// Ошибка прогрева не фатальна — движок остаётся рабочим.
    pub fn warm_up(&mut self) {
        let started = std::time::Instant::now();

        let rgb = synthetic_warmup_image();

        match self.engine.run_image_timed(&rgb) {
            Ok(timed) => {
                // Важно, чтобы rec_inference_ms > 0: значит за прогрев
                // отработали ОБЕ модели (det и rec) и обе сессии готовы.
                println!(
                    "OCR warm-up finished in {} ms (det {:.0} ms, rec {:.0} ms, {} lines)",
                    started.elapsed().as_millis(),
                    timed.timings.det_inference_ms,
                    timed.timings.rec_inference_ms,
                    timed.output.lines.len()
                );
            }
            Err(error) => println!("OCR warm-up failed: {error:#}"),
        }
    }

    fn recognize_rgb(&mut self, rgb: RgbImage) -> Result<Vec<OcrBox>> {
        let timed = self
            .engine
            .run_image_timed(&rgb)
            .context("PP-OCRv5 OCR inference failed")?;

        let mut boxes = Vec::new();

        for line in timed.output.lines {
            let text = line.text.trim();

            if text.is_empty() {
                continue;
            }

            append_word_boxes(&mut boxes, text, line.bbox.points, line.score);
        }

        // Не-латинский мусор выбрасывается ДО кэша и hit-test'а: клик рядом
        // с русской строкой не должен цеплять её ближайшим боксом.
        let before_filter = boxes.len();

        boxes.retain(|ocr_box| is_acceptable_ocr_text(&ocr_box.text));

        if boxes.len() != before_filter {
            println!(
                "OCR filter: dropped {} non-Latin/garbage boxes",
                before_filter - boxes.len()
            );
        }

        Ok(boxes)
    }
}

/// Читает f32-переменную окружения с проверкой диапазона.
///
/// Вне диапазона (и при непарсуемом значении) тихо берётся дефолт —
/// опечатка в env не должна ронять запуск OCR-движка.
fn env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && (min..=max).contains(value))
        .unwrap_or(default)
}

/// Хеш RGB-буфера кропа для кэша результатов.
///
/// SipHash по ~4 МБ пикселей стоит единицы миллисекунд — на фоне
/// det+rec в секунды это бесплатно, а вероятность коллизии 1/2^64
/// для практических целей ничтожна.
fn hash_rgb_image(rgb: &RgbImage) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    (rgb.width(), rgb.height()).hash(&mut hasher);
    rgb.as_raw().hash(&mut hasher);

    hasher.finish()
}

/// Белый холст с «текстом», отрисованным встроенным пиксельным шрифтом 5x7.
///
/// Слишком синтетические узоры (полосы, прямоугольники) DBNet-детектор
/// не считает текстом и не находит — тогда rec-модель в прогреве не
/// участвует и её сессия остаётся холодной. Буквенные формы детектор
/// находит надёжно, поэтому за один прогрев отрабатывают обе модели.
fn synthetic_warmup_image() -> RgbImage {
    // Размер и плотность реального кропа (1440x900, плотный текст).
    //
    // rec-модель принимает батчи ДИНАМИЧЕСКОЙ ширины (48 x ratio самой
    // широкой строки батча), и каждая новая форма заставляет ONNX
    // заново планировать граф и аллоцировать буферы. Узкая прогревочная
    // картинка покрывала только узкие формы — первый реальный экран
    // всё равно платил за планирование широких. Прогреваем худшим
    // случаем: полноразмерный «терминальный» кадр.
    synthetic_text_image(1440, 900, 34)
}

/// «Терминальный» текст пиксельным шрифтом 5x7: строки через row_step
/// пикселей, каждая заполняет ширину холста повтором фразы.
///
/// Служит и прогреву, и perf-тесту: плотность строк подбирается
/// аргументом, чтобы приблизить нагрузку к реальному экрану.
fn synthetic_text_image(width: u32, height: u32, row_step: u32) -> RgbImage {
    /// Глифы 5x7: строки сверху вниз, биты слева направо (MSB — левый столбец).
    const GLYPHS: &[(char, [u8; 7])] = &[
        ('A', [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        ('C', [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E]),
        ('D', [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E]),
        ('E', [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F]),
        ('H', [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11]),
        ('I', [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E]),
        ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F]),
        ('M', [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11]),
        ('N', [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11]),
        ('O', [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        ('P', [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10]),
        ('R', [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11]),
        ('S', [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E]),
        ('T', [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E]),
        ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A]),
        ('X', [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11]),
    ];

    const SCALE: u32 = 3;

    const PHRASE: &str = "SAMPLE TEXT OCR WARMUP MAIN IDEA";

    let mut image = RgbImage::from_pixel(width, height, image::Rgb([252, 252, 252]));

    let mut top = 36u32;

    while top + 7 * SCALE <= height.saturating_sub(36) {
        let mut x = 36u32;

        for character in PHRASE.chars().cycle() {
            // Ширина символа — 5 колонок глифа + 3 зазора, умноженные на масштаб.
            if x + 8 * SCALE > width.saturating_sub(36) {
                break;
            }

            if character != ' ' {
                if let Some((_, glyph)) = GLYPHS.iter().find(|(name, _)| *name == character) {
                    for (glyph_row, bits) in glyph.iter().enumerate() {
                        for column in 0..5u32 {
                            if bits & (1 << (4 - column)) == 0 {
                                continue;
                            }

                            for dy in 0..SCALE {
                                for dx in 0..SCALE {
                                    image.put_pixel(
                                        x + column * SCALE + dx,
                                        top + glyph_row as u32 * SCALE + dy,
                                        image::Rgb([30, 30, 30]),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            x += 8 * SCALE;
        }

        top += row_step;
    }

    image
}

/// Converts one line-level OCR result into word-level boxes.
///
/// The OCR engine gives us one polygon for the entire line. We cannot
/// obtain true character boxes from that result, so the best we can do
/// is estimate word positions from the horizontal layout.
///
/// Unlike the previous implementation, this function:
///
/// - works entirely with Unicode characters;
/// - accounts for whitespace;
/// - does not perform another OCR inference;
/// - keeps punctuation attached to its word;
/// - gives spaces their own estimated width instead of silently
///   assigning them to the following word.
fn append_word_boxes(boxes: &mut Vec<OcrBox>, text: &str, points: [[f32; 2]; 4], confidence: f32) {
    let words = split_words(text);

    if words.is_empty() {
        return;
    }

    // A single OCR word gets the complete line polygon.
    if words.len() == 1 {
        boxes.push(make_box(points, confidence, words[0].text.to_string()));

        return;
    }

    let total_chars = text.chars().count();

    if total_chars == 0 {
        return;
    }

    /*
     * We estimate the horizontal position of each word using character
     * widths.
     *
     * Example:
     *
     *   "The rain stopped."
     *
     * becomes approximately:
     *
     *   |---The---| |--rain--| |----stopped.----|
     *
     * instead of simply dividing the whole bbox into equal pieces.
     */
    let mut cursor = 0usize;

    for word in words {
        // Find where this word starts in character coordinates.
        //
        // `word.start` is already a character index, not a byte index.
        cursor = word.start;

        let start_ratio = cursor as f32 / total_chars as f32;
        let end_ratio = word.end as f32 / total_chars as f32;

        let top_left = interpolate(points[0], points[1], start_ratio);
        let top_right = interpolate(points[0], points[1], end_ratio);

        let bottom_right = interpolate(points[3], points[2], end_ratio);
        let bottom_left = interpolate(points[3], points[2], start_ratio);

        boxes.push(OcrBox {
            points: [
                OcrPoint {
                    x: top_left.0,
                    y: top_left.1,
                },
                OcrPoint {
                    x: top_right.0,
                    y: top_right.1,
                },
                OcrPoint {
                    x: bottom_right.0,
                    y: bottom_right.1,
                },
                OcrPoint {
                    x: bottom_left.0,
                    y: bottom_left.1,
                },
            ],
            confidence,
            text: word.text.to_string(),
        });

        cursor = word.end;
    }
}

struct Word<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

/// Splits text into words while preserving their character positions.
///
/// Byte offsets are used only for slicing the UTF-8 string.
/// `start`/`end` are character positions and are therefore safe to
/// use for bbox calculations.
fn split_words(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();

    let mut word_start_byte: Option<usize> = None;
    let mut word_start_char: usize = 0;

    for (char_index, (byte_index, ch)) in text.char_indices().enumerate() {
        if ch.is_whitespace() {
            if let Some(start_byte) = word_start_byte.take() {
                words.push(Word {
                    start: word_start_char,
                    end: char_index,
                    text: &text[start_byte..byte_index],
                });
            }
        } else if word_start_byte.is_none() {
            word_start_byte = Some(byte_index);
            word_start_char = char_index;
        }
    }

    if let Some(start_byte) = word_start_byte {
        words.push(Word {
            start: word_start_char,
            end: text.chars().count(),
            text: &text[start_byte..],
        });
    }

    words
}

// ============================================================================
// NON-LATIN FILTER
// ============================================================================

/// Фильтр мусора в выдаче rec-модели.
///
/// Модель PPOCRV5_EN_MOBILE знает ТОЛЬКО латиницу. Всё прочее на кадре —
/// русские субтитры, CJK-иероглифы, арабица — она перемалывает в мусор,
/// который затем ломает сборку предложения (метрики ocr_metrics_saved_crops:
/// «Не сбегал бы» -> "He cgeran 6bl", «что-то» -> "4TO-TO", «НОВОЕ» -> "NOBO").
///
/// Два правила:
///
/// 1. Любая не-латинская буква Unicode (кириллица, CJK, арабица, ...) —
///    валидного английского текста с таким символом не бывает.
/// 2. Цифра, прижатая к букве внутри токена ("6bl", "cgeran6bl", "4TO-TO") —
///    фирменный след транслитерации кириллицы lookalike-символами. Легитимные
///    алфавитно-цифровые токены ("2nd", "MP3", "4K", "1080p") разрешены
///    white-list'ом (см. alphanumeric_token_is_allowed).
///
/// Известное ограничение: чисто прописной мусор вида "NOBO" / "4TO"
/// неотличим от аббревиатур ("MP3", "4K") и проходит фильтр — LLM
/// устойчив к единичному такому токену в контексте.
fn is_acceptable_ocr_text(text: &str) -> bool {
    !has_non_latin_letters(text) && !has_embedded_digits(text)
}

/// Буквы латиницы, включая акцентированные (café, naïve, señor).
fn is_latin_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic()
        || matches!(ch, '\u{00C0}'..='\u{00FF}' | '\u{0100}'..='\u{024F}')
}

fn has_non_latin_letters(text: &str) -> bool {
    text.chars().any(|ch| ch.is_alphabetic() && !is_latin_letter(ch))
}

fn has_embedded_digits(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();

    for (index, &ch) in chars.iter().enumerate() {
        if !ch.is_ascii_digit() {
            continue;
        }

        let previous_is_letter =
            index > 0 && chars[index - 1].is_ascii_alphabetic();

        let next_is_letter =
            index + 1 < chars.len() && chars[index + 1].is_ascii_alphabetic();

        if (previous_is_letter || next_is_letter)
            && !alphanumeric_token_is_allowed(&chars)
        {
            return true;
        }
    }

    false
}

/// White-list легитимных алфавитно-цифровых токенов.
///
/// Токен должен целиком состоять из опционального буквенного префикса,
/// цифрового ядра и опционального буквенного суффикса:
///
///   "MP3", "F15", "v2"  — буквы спереди, цифры сзади;
///   "1080p", "3D"       — 2+ цифры спереди, 0-2 буквы сзади;
///   "4K", "4TO"         — одна цифра спереди + ПРОПИСНЫЕ буквы сзади
///                         ("6bl" с строчным суффиксом не проходит);
///   "2nd", "3rd", "4th" — порядковые.
///
/// Всё смешанное сверх схемы ("cgeran6bl", "4TO-TO") — мусор.
fn alphanumeric_token_is_allowed(chars: &[char]) -> bool {
    let mut index = 0;

    let mut letters_before = 0;

    while index < chars.len() && chars[index].is_ascii_alphabetic() {
        letters_before += 1;
        index += 1;
    }

    let mut digits = 0;

    while index < chars.len() && chars[index].is_ascii_digit() {
        digits += 1;
        index += 1;
    }

    let mut letters_after = 0;

    while index < chars.len() && chars[index].is_ascii_alphabetic() {
        letters_after += 1;
        index += 1;
    }

    if index != chars.len() {
        return false;
    }

    if letters_before > 0 && digits > 0 && letters_after == 0 {
        return true;
    }

    if letters_before == 0 && digits >= 2 && letters_after <= 2 {
        return true;
    }

    if letters_before == 0
        && digits == 1
        && letters_after >= 1
        && letters_after <= 3
        && chars[chars.len() - letters_after..]
            .iter()
            .all(|ch| ch.is_ascii_uppercase())
    {
        return true;
    }

    if letters_before == 0 && digits >= 1 && letters_after <= 2 {
        let suffix: String = chars[chars.len() - letters_after..].iter().collect();

        if matches!(suffix.to_ascii_lowercase().as_str(), "st" | "nd" | "rd" | "th") {
            return true;
        }
    }

    false
}

fn make_box(points: [[f32; 2]; 4], confidence: f32, text: String) -> OcrBox {
    OcrBox {
        points: [
            OcrPoint {
                x: points[0][0],
                y: points[0][1],
            },
            OcrPoint {
                x: points[1][0],
                y: points[1][1],
            },
            OcrPoint {
                x: points[2][0],
                y: points[2][1],
            },
            OcrPoint {
                x: points[3][0],
                y: points[3][1],
            },
        ],
        confidence,
        text,
    }
}

fn interpolate(a: [f32; 2], b: [f32; 2], ratio: f32) -> (f32, f32) {
    (a[0] + (b[0] - a[0]) * ratio, a[1] + (b[1] - a[1]) * ratio)
}

fn image_to_rgb_image(image: &Image) -> Result<RgbImage> {
    let expected_len = image.width as usize * image.height as usize * 3;

    if image.data.len() != expected_len {
        anyhow::bail!(
            "invalid RGB image buffer: expected {} bytes, got {}",
            expected_len,
            image.data.len()
        );
    }

    RgbImage::from_raw(image.width, image.height, image.data.clone())
        .context("failed to construct RgbImage from captured screen")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("app_core")
            .join("ocr")
            .join("ppocrv5-en")
    }

    /// A/B-проверка на реальном сохранённом кропе (качество + тайминг
    /// + попадание в кэш при повторном вызове):
    ///
    ///   ARMADILLO_TEST_IMAGE=/path/to/ocr_crop_*.png \
    ///     cargo test --release --lib -- --ignored ocr_real_crop --nocapture
    #[test]
    #[ignore]
    fn ocr_real_crop_test() {
        let path = std::env::var("ARMADILLO_TEST_IMAGE")
            .expect("set ARMADILLO_TEST_IMAGE to a saved ocr_crop_*.png");

        let rgb = image::open(&path)
            .unwrap_or_else(|error| panic!("failed to open {path}: {error}"))
            .to_rgb8();

        let crop = crate::app_core::lookup::image::Image {
            width: rgb.width(),
            height: rgb.height(),
            data: rgb.into_raw(),
        };

        let mut engine = OcrEngine::new(model_dir()).expect("failed to init OCR engine");

        // Первый прогон — аллокация буферов под динамические формы батчей.
        let started = std::time::Instant::now();

        let _ = engine.recognize(&crop).expect("OCR failed");

        println!("First run: {} ms", started.elapsed().as_millis());

        // Второй прогон — steady-state, печатаем_split по фазам.
        let started = std::time::Instant::now();

        let timed = engine
            .engine
            .run_image_timed(&image::RgbImage::from_raw(
                crop.width,
                crop.height,
                crop.data.clone(),
            )
            .expect("bad crop buffer"))
            .expect("OCR failed");

        println!(
            "Real crop {}x{}: {} lines in {} ms | det prep {:.0} inf {:.0} post {:.0} | crop {:.0} | rec prep {:.0} inf {:.0} decode {:.0} ms",
            crop.width,
            crop.height,
            timed.output.lines.len(),
            started.elapsed().as_millis(),
            timed.timings.det_preprocess_ms,
            timed.timings.det_inference_ms,
            timed.timings.det_postprocess_ms,
            timed.timings.crop_ms,
            timed.timings.rec_preprocess_ms,
            timed.timings.rec_inference_ms,
            timed.timings.rec_decode_ms,
        );

        let boxes = engine.recognize(&crop).expect("OCR failed");

        println!("Word boxes: {}", boxes.len());

        let text: Vec<&str> = boxes.iter().map(|item| item.text.as_str()).collect();

        println!("Recognized: {}", text.join(" "));
    }

    /// Проверка прогрева: за один вызов warm_up должны отработать
    /// и det, и rec (rec_inference_ms > 0). Запуск:
    ///
    ///   cargo test --lib -- --ignored ocr_warmup --nocapture
    #[test]
    #[ignore]
    fn ocr_warmup_exercises_det_and_rec() {
        let mut engine = OcrEngine::new(model_dir()).expect("failed to init OCR engine");

        engine.warm_up();
    }

    /// Smoke-тест производительности OCR на изображении размера кропа.
    ///
    /// Запускается вручную (нужны ONNX-модели в src/app_core/ocr/ppocrv5-en):
    ///
    ///   cargo test -p armadillo-learn-desktop --release -- --ignored ocr_smoke --nocapture
    ///
    /// Debug-сборка сильно искажает препроцессинг — запускать только release.
    #[test]
    #[ignore]
    fn ocr_smoke_test_region_performance() {
        let mut engine = OcrEngine::new(model_dir()).expect("failed to init OCR engine");

        // Синтетический «скриншот» размера OCR-кропа: плотный
        // пиксельный текст, как на терминале или в IDE. Это главный
        // стресс-случай rec-инференса: много боксов и широкие кропы.
        let rgb = synthetic_text_image(1440, 900, 28);

        let image = Image {
            width: rgb.width(),
            height: rgb.height(),
            data: rgb.into_raw(),
        };

        // Первый прогон — прогрев сессий ONNX Runtime.
        let started = std::time::Instant::now();

        let warmup = engine.recognize(&image).expect("warm-up OCR failed");

        println!(
            "Warm-up: {}ms, {} boxes",
            started.elapsed().as_millis(),
            warmup.len()
        );

        // Измерительный прогон.
        let started = std::time::Instant::now();

        let boxes = engine.recognize(&image).expect("measured OCR failed");

        println!(
            "Measured: {}ms, {} boxes",
            started.elapsed().as_millis(),
            boxes.len()
        );
    }

    /// Печатает метрики распознавания одного изображения.
    ///
    /// Два среза: word-боксы из recognize() (как их видит пайплайн lookup)
    /// и line-уровень с confidence из run_image_timed (как их вернул детектор).
    /// Расхождение между «на изображении видно N строк» и напечатанным —
    /// прямой сигнал о проблеме детекции; пустой/обрывочный text строки —
    /// ошибка recognition.
    fn ocr_report(label: &str, engine: &mut OcrEngine, image: &Image, max_lines: usize) {
        let started = std::time::Instant::now();

        let boxes = engine.recognize(image).expect("OCR failed");

        let recognize_ms = started.elapsed().as_millis();

        let rgb = image::RgbImage::from_raw(image.width, image.height, image.data.clone())
            .expect("invalid image buffer");

        let timed = engine
            .engine
            .run_image_timed(&rgb)
            .expect("timed OCR failed");

        println!(
            "--- {label}: {}x{} | recognize {recognize_ms} ms, {} word boxes | timed: pipeline {:.0} ms, det prep {:.0} inf {:.0} post {:.0} | crop {:.0} | rec prep {:.0} inf {:.0} decode {:.0} ms | {} lines ---",
            image.width,
            image.height,
            boxes.len(),
            timed.timings.pipeline_preprocess_ms,
            timed.timings.det_preprocess_ms,
            timed.timings.det_inference_ms,
            timed.timings.det_postprocess_ms,
            timed.timings.crop_ms,
            timed.timings.rec_preprocess_ms,
            timed.timings.rec_inference_ms,
            timed.timings.rec_decode_ms,
            timed.output.lines.len(),
        );

        for (index, line) in timed.output.lines.iter().enumerate() {
            if index == max_lines {
                println!(
                    "  ... (+{} more lines, total {})",
                    timed.output.lines.len() - max_lines,
                    timed.output.lines.len()
                );

                break;
            }

            println!("  [{:.3}] {}", line.score, line.text.trim());
        }
    }

    /// Фильтр текста: латиница (включая акценты, числа и легитимные
    /// алфавитно-цифровые токены) проходит; Unicode не-латиницы и
    /// транслитерированная кириллица lookalike-символами — нет.
    #[test]
    fn text_filter_separates_latin_from_garbage() {
        // Латиница, пунктуация, числа, акцентированная латиница, white-list.
        for text in [
            "dodge",
            "don't",
            "10:30",
            "70%",
            "café",
            "naïve",
            "2nd",
            "MP3",
            "F15",
            "4K",
            "1080p",
            "1st",
            "v2",
            "3D",
        ] {
            assert!(is_acceptable_ocr_text(text), "«{text}» должно остаться");
        }

        // Прямая не-латиница Unicode: EN-модель не может её выдать осмысленно.
        for text in ["Привет", "мир?", "日本語", "한국어", "مرحبا"] {
            assert!(!is_acceptable_ocr_text(text), "«{text}» должно отфильтроваться");
        }

        // Транслитерированная кириллица lookalike-символами (реальный вывод
        // EN-модели на русских субтитрах, см. метрики). Чисто прописной мусор
        // вида "NOBO"/"4TO" неотличим от аббревиатур и остаётся (задокументировано).
        for text in ["6bl", "cgeran6bl", "4TO-TO"] {
            assert!(!is_acceptable_ocr_text(text), "«{text}» должно отфильтроваться");
        }
    }

    /// Метрики OCR на сохранённых реальных кропах (screenshots/ocr_crop_*.png).
    ///
    /// Воспроизводит «на кропе видны хорошие субтитры, а OCR вернул одно
    /// слово»: печатает ВСЕ найденные строки с confidence и тайминги по фазам.
    ///
    /// Запуск:
    ///   cargo test --lib -- --ignored ocr_metrics_saved_crops --nocapture
    ///
    /// Сколько последних кропов брать (по умолчанию 4):
    ///   ARMADILLO_OCR_METRICS_CROPS=8 cargo test ...
    #[test]
    #[ignore]
    fn ocr_metrics_saved_crops() {
        let count = std::env::var("ARMADILLO_OCR_METRICS_CROPS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(4);

        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("screenshots");

        let mut crops: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .expect("screenshots dir missing")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .map_or(false, |name| name.to_string_lossy().starts_with("ocr_crop_"))
            })
            .collect();

        crops.sort_by_key(|path| path.metadata().unwrap().modified().unwrap());

        let crops: Vec<std::path::PathBuf> = if crops.len() > count {
            crops[crops.len() - count..].to_vec()
        } else {
            crops
        };

        assert!(!crops.is_empty(), "no ocr_crop_*.png in {}", dir.display());

        let mut engine = OcrEngine::new(model_dir()).expect("failed to init OCR engine");

        // Прогрев сессий ONNX на первом кропе, чтобы метрики каждого кропа
        // были steady-state (первый инференс платит за аллокацию буферов).
        let first = image::open(&crops[0])
            .unwrap_or_else(|error| panic!("failed to open {}: {error}", crops[0].display()))
            .to_rgb8();

        let _ = engine
            .engine
            .run_image_timed(&first)
            .expect("warm-up OCR failed");

        for path in &crops {
            let rgb = image::open(path)
                .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()))
                .to_rgb8();

            let image = Image {
                width: rgb.width(),
                height: rgb.height(),
                data: rgb.into_raw(),
            };

            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();

            ocr_report(&name, &mut engine, &image, usize::MAX);
        }
    }

    /// Метрики OCR ПОЛНОГО экрана против центрального кропа того же кадра.
    ///
    /// Вопрос эксперимента: находит ли детектор больше текста на полном
    /// кадре, чем в области вокруг клика. Полный кадр при стороне > 2000 px
    /// дополнительно сжимается pipeline-ресайзом rapidocr
    /// (RapidOcrConfig::min_side_len..max_side_len) — сравнивать нужно не
    /// только число строк, но и текст.
    ///
    /// Запуск:
    ///   cargo test --lib -- --ignored ocr_metrics_fullscreen_vs_region --nocapture
    ///
    /// Размер региона (по умолчанию 1440x900, как OCR_CROP_* в capture.rs):
    ///   ARMADILLO_OCR_METRICS_CROP_W=1920 ARMADILLO_OCR_METRICS_CROP_H=1080 cargo test ...
    #[test]
    #[ignore]
    fn ocr_metrics_fullscreen_vs_region() {
        use screenshots::Screen;

        let screen = Screen::from_point(100, 100).expect("failed to find screen");

        let display = screen.display_info;

        println!(
            "Display: {}x{} (scale x{})",
            display.width, display.height, display.scale_factor
        );

        let shot = screen.capture().expect("failed to capture screen");

        let width = shot.width() as usize;
        let height = shot.height() as usize;

        let pixels = shot.as_raw();

        assert_eq!(pixels.len(), width * height * 4, "unexpected frame format");

        // BGRA -> RGB полного кадра (то же преобразование, что в capture.rs).
        let mut full_rgb = Vec::with_capacity(width * height * 3);

        for pixel in pixels.chunks_exact(4) {
            full_rgb.push(pixel[2]); // R
            full_rgb.push(pixel[1]); // G
            full_rgb.push(pixel[0]); // B
        }

        let full = Image {
            width: width as u32,
            height: height as u32,
            data: full_rgb,
        };

        // Кадр сохраняется для визуальной сверки «что видел OCR».
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("screenshots");

        let _ = std::fs::create_dir_all(&dir);

        let frame_path = dir.join(format!(
            "ocr_fullscreen_{}.png",
            crate::app_core::lookup::time::now_ms()
        ));

        if let Ok(png) = crate::app_core::lookup::image::encode_png(&full) {
            let _ = std::fs::write(&frame_path, png);

            println!("Full frame saved: {}", frame_path.display());
        }

        let crop_width = std::env::var("ARMADILLO_OCR_METRICS_CROP_W")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(1440)
            .min(width);

        let crop_height = std::env::var("ARMADILLO_OCR_METRICS_CROP_H")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(900)
            .min(height);

        let left = (width - crop_width) / 2;
        let top = (height - crop_height) / 2;

        // Центральный кроп из ТОГО ЖЕ кадра — как в capture_screen.
        let mut region_rgb = Vec::with_capacity(crop_width * crop_height * 3);

        for row in top..top + crop_height {
            let row_start = (row * width + left) * 4;

            let row_pixels = &pixels[row_start..row_start + crop_width * 4];

            for pixel in row_pixels.chunks_exact(4) {
                region_rgb.push(pixel[2]); // R
                region_rgb.push(pixel[1]); // G
                region_rgb.push(pixel[0]); // B
            }
        }

        let region = Image {
            width: crop_width as u32,
            height: crop_height as u32,
            data: region_rgb,
        };

        let mut engine = OcrEngine::new(model_dir()).expect("failed to init OCR engine");

        // Прогрев обеих форм входа: полный кадр и кроп дают разные формы
        // тензора детектора, каждая первая итерация платит за аллокации.
        // run_image_timed используется напрямую, чтобы не трогать кэш recognize.
        let region_rgb_image = image::RgbImage::from_raw(region.width, region.height, region.data.clone())
            .expect("invalid region buffer");

        let full_rgb_image = image::RgbImage::from_raw(full.width, full.height, full.data.clone())
            .expect("invalid full buffer");

        let _ = engine
            .engine
            .run_image_timed(&region_rgb_image)
            .expect("warm-up OCR failed");

        let _ = engine
            .engine
            .run_image_timed(&full_rgb_image)
            .expect("warm-up OCR failed");

        ocr_report("FULL SCREEN", &mut engine, &full, 14);

        ocr_report("CENTER REGION", &mut engine, &region, usize::MAX);
    }
}
