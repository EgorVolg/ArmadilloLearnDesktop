use super::image::Image;
use super::region::Region;

/// Ошибки, которые могут возникнуть при обрезке изображения.
#[derive(Debug)]
pub enum CropError {
    /// Область полностью или частично выходит
    /// за границы исходного изображения.
    OutOfBounds,
    /// Передана область с нулевой шириной или высотой.
    EmptyRegion,
    /// Внутренняя ошибка создания нового изображения.
    InvalidImage(String),
}

/// Обрезает изображение по заданной области.
pub fn crop(image: &Image, region: Region) -> Result<Image, CropError> {
    if region.width == 0 || region.height == 0 {
        return Err(CropError::EmptyRegion);
    }

    // Преобразуем координаты в i64,
    // чтобы безопасно выполнять арифметику
    // даже если в будущем Region будет использовать
    // отрицательные координаты.
    let x = region.x as i64;
    let y = region.y as i64;

    let right = x + region.width as i64;
    let bottom = y + region.height as i64;

    let image_width = image.width as i64;
    let image_height = image.height as i64;

    // Проверяем, что весь Region находится внутри изображения.
    if x < 0 || y < 0 || right > image_width || bottom > image_height {
        return Err(CropError::OutOfBounds);
    }

    let width = region.width as usize;
    let height = region.height as usize;

    // RGB = 3 байта на пиксель.
    let bytes_per_pixel = Image::bytes_per_pixel();

    // Размер одной строки исходного изображения.
    let source_stride = image.width as usize * bytes_per_pixel;

    // Размер одной строки нового изображения.
    let cropped_stride = width * bytes_per_pixel;

    let mut data = Vec::with_capacity(height * cropped_stride);

    for row in 0..height {
        // Начало нужной строки в исходном изображении.
        let source_start =
            (region.y as usize + row) * source_stride + region.x as usize * bytes_per_pixel;

        let source_end = source_start + cropped_stride;

        // Копируем только нужную часть строки.
        data.extend_from_slice(&image.data[source_start..source_end]);
    }

    Image::new(region.width, region.height, data).map_err(CropError::InvalidImage)
}
