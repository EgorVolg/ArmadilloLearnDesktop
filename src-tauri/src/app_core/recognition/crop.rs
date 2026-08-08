use super::region::Region;

/// Вырезает указанную область из изображения.
///
/// Возвращает `None`, если область полностью находится
/// за пределами исходного изображения.
pub fn crop(image: &Image, region: Region) -> Option<Image> {
    // Проверяем, что исходное изображение вообще содержит данные.
    if image.is_empty() {
        return None;
    }

    // Получаем размеры исходного изображения.
    let image_width = image.width as i32;
    let image_height = image.height as i32;

    // Ограничиваем левую и верхнюю границы,
    // чтобы они не выходили за пределы изображения.
    let left = region.x.max(0);
    let top = region.y.max(0);

    // Аналогично ограничиваем правую и нижнюю границы.
    let right = (region.x + region.width as i32).min(image_width);
    let bottom = (region.y + region.height as i32).min(image_height);

    // Если после пересечения область оказалась пустой,
    // вырезать нечего.
    if left >= right || top >= bottom {
        return None;
    }

    // Вычисляем фактический размер обрезанного изображения.
    let width = (right - left) as u32;
    let height = (bottom - top) as u32;

    // Определяем, сколько байт занимает один пиксель.
    let bytes_per_pixel = match image.format {
        PixelFormat::Rgb8 => 3,
        PixelFormat::Rgba8 => 4,
    };

    // Создаём буфер для пикселей нового изображения.
    let mut data = Vec::with_capacity((width * height * bytes_per_pixel) as usize);

    // Копируем нужные строки исходного изображения
    // в новый буфер.
    for y in top..bottom {
        // Вычисляем позицию начала нужного участка строки.
        let start = ((y * image_width + left) * bytes_per_pixel as i32) as usize;

        // Вычисляем конец нужного участка строки.
        let end = start + (width * bytes_per_pixel) as usize;

        // Добавляем пиксели этой строки в новое изображение.
        data.extend_from_slice(&image.data[start..end]);
    }

    // Создаём и возвращаем новое обрезанное изображение.
    Some(Image::new(width, height, image.format, data))
}
