use crate::app_core::recognition::image::Image;

/// Подготовленное изображение для detection-модели.
///
/// Данные хранятся в формате:
///
/// [R plane][G plane][B plane]
///
/// То есть фактически:
///
/// [1, 3, H, W]
pub struct DetectionInput {
    /// Float32 данные tensor.
    pub data: Vec<f32>,

    /// Ширина изображения.
    pub width: usize,

    /// Высота изображения.
    pub height: usize,
}

/// Преобразует внутренний `Image` в формат,
/// необходимый ONNX detection-модели.
///
/// Пока здесь выполняется только преобразование:
///
/// RGB interleaved:
/// RGB RGB RGB ...
///
/// в:
///
/// RRR... GGG... BBB...
///
/// Нормализацию PP-OCRv5 добавим после успешного
/// запуска первого inference.
pub fn preprocess(image: &Image) -> DetectionInput {
    let width = image.width as usize;
    let height = image.height as usize;

    let plane_size = width * height;

    // Выделяем три отдельных канала:
    //
    // [R...][G...][B...]
    let mut data = vec![0.0f32; plane_size * 3];

    for y in 0..height {
        for x in 0..width {
            // В исходном Image RGB хранится последовательно:
            //
            // [R][G][B][R][G][B]...
            let source_index = (y * width + x) * 3;

            // В NCHW каждый канал занимает отдельную плоскость.
            let pixel_index = y * width + x;

            let r = image.data[source_index] as f32;
            let g = image.data[source_index + 1] as f32;
            let b = image.data[source_index + 2] as f32;

            data[pixel_index] = r;
            data[plane_size + pixel_index] = g;
            data[plane_size * 2 + pixel_index] = b;
        }
    }

    DetectionInput {
        data,
        width,
        height,
    }
}
