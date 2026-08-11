use crate::app_core::recognition::image::Image;

/// Подготовленное изображение для PP-OCRv5 detection.
///
/// На выходе получаем Float32 tensor в формате:
///
/// [1, 3, H, W]
///
/// с нормализацией, совместимой с preprocessing
/// оригинальной PP-OCRv5 mobile detection модели.
pub struct DetectionInput {
    /// Float32 данные tensor в формате CHW.
    pub data: Vec<f32>,

    /// Ширина подготовленного изображения.
    pub width: usize,

    /// Высота подготовленного изображения.
    pub height: usize,
}

/// Ограничивает максимальную длинную сторону изображения.
///
/// PP-OCRv5 mobile det использует resize_long = 960.
///
/// Важно:
/// мы сохраняем aspect ratio изображения.
/// Рассчитывает размер изображения для PP-OCRv5 detection.
///
/// PP-OCRv5 detection использует ограничения по длинной стороне
/// и архитектуру, которая уменьшает пространственные размеры
/// изображения в несколько раз.
///
/// Поэтому высота и ширина входного tensor должны быть
/// кратны 32.
///
/// Например:
///
/// 800x400 → 800x416
///
/// 800x416 → 800x416
///
/// Это важно, потому что при размере 400 модель получает
/// промежуточный размер 25, тогда как для корректного
/// прохождения через сеть нужен размер 26.
fn resize_size(width: usize, height: usize) -> (usize, usize) {
    let max_side = width.max(height);

    // Сначала ограничиваем длинную сторону 960 пикселями.
    let (scaled_width, scaled_height) = if max_side <= 960 {
        (width, height)
    } else {
        let scale = 960.0 / (max_side as f32);

        let new_width = ((width as f32) * scale).round() as usize;
        let new_height = ((height as f32) * scale).round() as usize;

        (new_width.max(1), new_height.max(1))
    };

    // PP-OCR detection должен получать размеры,
    // совместимые с downsampling внутри модели.
    //
    // Округляем вверх до ближайшего числа,
    // кратного 32.
    let width = ((scaled_width + 31) / 32) * 32;
    let height = ((scaled_height + 31) / 32) * 32;

    (width.max(32), height.max(32))
}

/// Изменяет размер RGB изображения.
///
/// Используем простую bilinear interpolation.
fn resize_rgb(image: &Image, new_width: usize, new_height: usize) -> Vec<u8> {
    let src_width = image.width as usize;
    let src_height = image.height as usize;

    let mut output = vec![0u8; new_width * new_height * 3];

    let scale_x = (src_width as f32) / (new_width as f32);
    let scale_y = (src_height as f32) / (new_height as f32);

    for y in 0..new_height {
        let src_y = ((y as f32) + 0.5) * scale_y - 0.5;

        let y0 = src_y.floor().max(0.0) as usize;
        let y1 = (y0 + 1).min(src_height - 1);

        let fy = src_y - (y0 as f32);

        for x in 0..new_width {
            let src_x = ((x as f32) + 0.5) * scale_x - 0.5;

            let x0 = src_x.floor().max(0.0) as usize;
            let x1 = (x0 + 1).min(src_width - 1);

            let fx = src_x - (x0 as f32);

            let p00 = (y0 * src_width + x0) * 3;
            let p01 = (y0 * src_width + x1) * 3;
            let p10 = (y1 * src_width + x0) * 3;
            let p11 = (y1 * src_width + x1) * 3;

            let dst = (y * new_width + x) * 3;

            for channel in 0..3 {
                let top =
                    (image.data[p00 + channel] as f32) * (1.0 - fx) +
                    (image.data[p01 + channel] as f32) * fx;

                let bottom =
                    (image.data[p10 + channel] as f32) * (1.0 - fx) +
                    (image.data[p11 + channel] as f32) * fx;

                let value = top * (1.0 - fy) + bottom * fy;

                output[dst + channel] = value.round() as u8;
            }
        }
    }

    output
}

/// Преобразует RGB Image в вход PP-OCRv5.
///
/// Выполняются шаги:
///
/// RGB
///   ↓
/// resize_long = 960
///   ↓
/// BGR
///   ↓
/// scale = 1 / 255
///   ↓
/// mean / std normalization
///   ↓
/// HWC → CHW
pub fn preprocess(image: &Image) -> DetectionInput {
    let original_width = image.width as usize;
    let original_height = image.height as usize;

    // Вычисляем новый размер с сохранением
    // исходного aspect ratio.
    let (width, height) = resize_size(original_width, original_height);

    println!("Detection resize: {}x{} -> {}x{}", original_width, original_height, width, height);

    // Resize изображения.
    let resized = resize_rgb(image, width, height);

    let plane_size = width * height;

    // Выделяем три CHW-плоскости.
    let mut data = vec![0.0f32; plane_size * 3];

    // Параметры NormalizeImage из PP-OCRv5.
    let mean = [0.485f32, 0.456f32, 0.406f32];

    let std = [0.229f32, 0.224f32, 0.225f32];

    for y in 0..height {
        for x in 0..width {
            let source_index = (y * width + x) * 3;

            let pixel_index = y * width + x;

            // Исходный Image хранится как RGB.
            let r = (resized[source_index] as f32) / 255.0;

            let g = (resized[source_index + 1] as f32) / 255.0;

            let b = (resized[source_index + 2] as f32) / 255.0;

            // Конфигурация PP-OCRv5 указывает
            // img_mode = BGR.
            //
            // Поэтому на вход модели передаём:
            // B, G, R.
            let b = (b - mean[0]) / std[0];
            let g = (g - mean[1]) / std[1];
            let r = (r - mean[2]) / std[2];

            data[pixel_index] = b;

            data[plane_size + pixel_index] = g;

            data[plane_size * 2 + pixel_index] = r;
        }
    }

    DetectionInput {
        data,
        width,
        height,
    }
}
