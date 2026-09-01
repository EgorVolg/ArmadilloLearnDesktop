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
            "OCR inference: intra_threads={intra_threads}, rec_batch={rec_batch_size}, pipeline=det+rec, ep={provider_label}"
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

        Ok(boxes)
    }
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
}
