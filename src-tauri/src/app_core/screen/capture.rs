use crate::app_core::lookup::image::{encode_png, Image};
use crate::app_core::lookup::time::now_ms;
use screenshots::Screen;
use std::path::{Path, PathBuf};

/// Размер (в физических пикселях) области вокруг клика,
/// которая вырезается из скриншота и отправляется в OCR.
///
/// Раньше OCR обрабатывал весь монитор: детектор PP-OCR гонял
/// инференс по всему экрану (2+ мегапикселя), хотя для lookup
/// достаточно региона вокруг клика — кликнутое слово плюс
/// предложение вокруг него с учётом переносов строк.
///
/// 1440x900 при высоте строки ~20-30px вмещает десятки строк,
/// чего с запасом хватает для MAX_SENTENCE_LINES = 12
/// в extract_sentence_context.
const OCR_CROP_WIDTH: i32 = 1440;
const OCR_CROP_HEIGHT: i32 = 900;

pub struct CapturedScreen {
    /// RGB-изображение области вокруг клика.
    pub image: Image,

    /// Левая верхняя точка вырезанной области
    /// в глобальной системе координат desktop.
    pub origin_x: i32,
    pub origin_y: i32,

    /// Координаты клика относительно
    /// верхнего левого угла вырезанной области.
    pub click_x: i32,
    pub click_y: i32,
}

pub fn capture_screen(click_x: i32, click_y: i32) -> Result<CapturedScreen, String> {
    let screen = Screen::from_point(click_x, click_y)
        .map_err(|error| format!("Failed to find screen at ({click_x}, {click_y}): {error}"))?;

    let display = screen.display_info;

    let screenshot = screen
        .capture()
        .map_err(|error| format!("Failed to capture screen: {error}"))?;

    let width = screenshot.width() as i32;
    let height = screenshot.height() as i32;

    let pixels = screenshot.as_raw();

    let expected_len = (width as usize) * (height as usize) * 4;

    if pixels.len() != expected_len {
        return Err(format!(
            "Invalid frame size: expected {}, got {}",
            expected_len,
            pixels.len()
        ));
    }

    let local_click_x = click_x - display.x;
    let local_click_y = click_y - display.y;

    // =========================================================
    // CROP AROUND CLICK
    // =========================================================
    //
    // Вырезаем ограниченную область вокруг клика вместо того,
    // чтобы отправлять весь монитор в OCR.
    //
    // Координаты клика теоретически могут выйти за границы
    // скриншота (многомониторные конфигурации), поэтому центр
    // кропа зажимается в границы изображения. Если клик реально
    // попал за пределы экрана, координата клика в кропе тоже
    // окажется вне кропа — пайплайн отработает как раньше
    // (ошибка "No OCR text region found under cursor").

    let center_x = local_click_x.clamp(0, width - 1);
    let center_y = local_click_y.clamp(0, height - 1);

    let crop_width = OCR_CROP_WIDTH.min(width);
    let crop_height = OCR_CROP_HEIGHT.min(height);

    let crop_left = (center_x - crop_width / 2).clamp(0, width - crop_width);
    let crop_top = (center_y - crop_height / 2).clamp(0, height - crop_height);

    // =========================================================
    // BGRA -> RGB (только вырезанная область)
    // =========================================================
    //
    // Конвертация всего монитора попиксельно занимала заметное
    // время в горячем пути — теперь конвертируем только кроп.

    let crop_width_usize = crop_width as usize;
    let width_usize = width as usize;

    let mut rgb = Vec::with_capacity((crop_width as usize) * (crop_height as usize) * 3);

    for row in crop_top..crop_top + crop_height {
        let row_start = (row as usize * width_usize + crop_left as usize) * 4;

        let row_pixels = &pixels[row_start..row_start + crop_width_usize * 4];

        for pixel in row_pixels.chunks_exact(4) {
            rgb.push(pixel[2]); // R
            rgb.push(pixel[1]); // G
            rgb.push(pixel[0]); // B
        }
    }

    println!(
        "Captured crop: {}x{} at monitor ({}, {}), click in crop: ({}, {})",
        crop_width,
        crop_height,
        crop_left,
        crop_top,
        local_click_x - crop_left,
        local_click_y - crop_top
    );

    let captured = CapturedScreen {
        image: Image {
            width: crop_width as u32,
            height: crop_height as u32,
            data: rgb,
        },
        origin_x: display.x + crop_left,
        origin_y: display.y + crop_top,
        click_x: local_click_x - crop_left,
        click_y: local_click_y - crop_top,
    };

    save_crop_debug(&captured.image);

    Ok(captured)
}

/// Диагностика: сохраняет вырезанную область в <project_root>/screenshots/,
/// чтобы видеть, что именно получает OCR (разбор промахов распознавания).
///
/// Ошибка сохранения не должна ломать lookup — только логируется.
fn save_crop_debug(image: &Image) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let project_root = manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let dir = project_root.join("screenshots");

    if let Err(error) = std::fs::create_dir_all(&dir) {
        println!(
            "Debug crop save skipped: cannot create {}: {error}",
            dir.display()
        );

        return;
    }

    let path = dir.join(format!("ocr_crop_{}.png", now_ms()));

    let result = encode_png(image).and_then(|png| {
        std::fs::write(&path, png).map_err(|error| format!("write failed: {error}"))
    });

    match result {
        Ok(_) => println!("Debug crop saved: {}", path.display()),
        Err(error) => println!("Debug crop save failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Запуск: cargo test --release --lib -- --ignored capture_smoke --nocapture
    ///
    /// Требует реального дисплея: делает захват вокруг точки (400, 300)
    /// и проверяет, что кроп сохранён в screenshots/ocr_crop_*.png.
    #[test]
    #[ignore]
    fn capture_smoke_test_saves_debug_crop() {
        let captured = capture_screen(400, 300).expect("capture_screen failed");

        assert_eq!(captured.image.data.len(), (captured.image.width * captured.image.height * 3) as usize);

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let dir = manifest_dir.parent().unwrap().join("screenshots");

        let mut crops: Vec<_> = std::fs::read_dir(&dir)
            .expect("screenshots dir missing")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.file_name().map_or(false, |name| name.to_string_lossy().starts_with("ocr_crop_")))
            .collect();

        assert!(!crops.is_empty(), "no ocr_crop_*.png found in {}", dir.display());

        crops.sort_by_key(|path| path.metadata().unwrap().modified().unwrap());

        let latest = crops.last().unwrap();

        let metadata = std::fs::metadata(latest).unwrap();

        println!("Latest crop: {} ({} bytes)", latest.display(), metadata.len());

        assert!(metadata.len() > 0, "saved crop is empty");
    }
}
