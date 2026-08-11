//////////////////////////!!!!!!!!!'////////////////////////
// !!!!!!!! Используем первый экран из списка. !!!!!!!!!!!!
//////////////////////////!!!!!!!!!'////////////////////////
/// 
/// 
/// 
/// 
/// Захватывает указанную область экрана.
///
/// Эта функция отвечает за получение изображения. 
use screenshots::Screen;

use super::image::Image;

/// Ошибки, которые могут возникнуть при захвате экрана.
#[derive(Debug)]
pub enum CaptureError {
    /// Не удалось получить список экранов.
    ScreensUnavailable(String),

    /// Не удалось захватить изображение.
    CaptureFailed(String),

    /// Полученный кадр имеет некорректные размеры.
    InvalidFrame(String),

    /// Не удалось создать наше внутреннее `Image`.
    InvalidImage(String),
}

/// Захватывает основной экран.
///
/// На выходе получаем RGB-изображение,
/// не зависящее от конкретного Windows API.
pub fn capture_screen() -> Result<Image, CaptureError> {
    // Получаем список доступных мониторов.
    let screens =
        Screen::all().map_err(|error| CaptureError::ScreensUnavailable(error.to_string()))?;

    let screen = screens
        .first()
        .ok_or_else(|| CaptureError::ScreensUnavailable("No screens found".to_string()))?;

    // Делаем screenshot выбранного экрана.
    let screenshot = screen
        .capture()
        .map_err(|error| CaptureError::CaptureFailed(error.to_string()))?;

    let width = screenshot.width();
    let height = screenshot.height();

    // Получаем исходный pixel buffer.
    let pixels = screenshot.as_raw();

    // Проверяем, что количество байт соответствует
    // ожидаемому формату BGRA.
    //
    // 4 байта на пиксель:
    //
    // B G R A
    let expected_len = width as usize * height as usize * 4;

    if pixels.len() != expected_len {
        return Err(CaptureError::InvalidFrame(format!(
            "Invalid frame size: expected {}, got {}",
            expected_len,
            pixels.len()
        )));
    }

    // Создаём RGB buffer.
    //
    // Было:
    //
    // B G R A | B G R A | B G R A
    //
    // Станет:
    //
    // R G B | R G B | R G B
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);

    for pixel in pixels.chunks_exact(4) {
        let b = pixel[0];
        let g = pixel[1];
        let r = pixel[2];

        // Alpha нам для OCR не нужен.
        rgb.push(r);
        rgb.push(g);
        rgb.push(b);
    }

    Image::new(width, height, rgb).map_err(CaptureError::InvalidImage)
}
