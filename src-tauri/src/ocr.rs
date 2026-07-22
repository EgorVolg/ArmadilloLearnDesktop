// Импорты для работы с изображениями: DynamicImage, ImageBuffer для создания/обработки растровых изображений, Rgba для пикселей
use image::{DynamicImage, ImageBuffer, Rgba};
// Крейт screenshots для захвата скриншотов с экрана
use screenshots::Screen;
// Cursor для записи PNG в память (Vec<u8>) без создания файла на диске
use std::io::Cursor;
// Импорты Windows Runtime API: WinRT-типы и трейты
use windows::{
    core::*,
    Graphics::Imaging::BitmapDecoder,
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};

/// Захватывает область экрана вокруг точки (x, y) и возвращает PNG-байты.
pub fn capture_area(x: i32, y: i32, width: u32, height: u32) -> Option<Vec<u8>> {
    // Получаем экран, на котором находится точка (x, y)
    let screen = Screen::from_point(x, y).ok()?;
    // Захватываем прямоугольную область с центром в (x, y) размерами width x height
    let img = screen
        .capture_area(x - width as i32 / 2, y - height as i32 / 2, width, height)
        .ok()?;

    let (w, h) = (img.width(), img.height());
    // Извлекаем сырые RGBA-байты из захваченного изображения
    let raw_data = img.into_raw(); // RGBA байты из screenshots::image

    // Создаём ImageBuffer из нашего крейта image (v0.25)
    // Принимает сырые данные и размеры; паникует, если размеры не совпадают
    let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(w, h, raw_data).expect("raw data should match dimensions");

    // Конвертируем в DynamicImage и кодируем в PNG
    let dynamic = DynamicImage::ImageRgba8(img_buffer);
    let mut png_bytes = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .ok()?;

    Some(png_bytes)
}

/// Принимает PNG-байты и возвращает распознанный текст через Windows OCR.
pub async fn ocr_from_png(png_bytes: Vec<u8>) -> Result<String> {
    // Создаём поток в памяти (InMemoryRandomAccessStream) для передачи в WinRT API
    // Создаём DataWriter для записи данных в поток
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    // Записываем PNG-байты в поток
    writer.WriteBytes(&png_bytes)?;
    // Сохраняем данные асинхронно: StoreAsync возвращает IAsyncOperation, ждём завершения
    writer.StoreAsync()?.await?;
    // Сбрасываем буфер записи
    writer.FlushAsync()?.await?;
    // Сбрасываем позицию потока на начало для чтения
    stream.Seek(0)?;

    // Создаём декодер PNG из потока: BitmapDecoder автоматически распознаёт формат
    let decoder =
        BitmapDecoder::CreateWithIdAsync(BitmapDecoder::PngDecoderId()?, &stream)?.await?;
    // Получаем SoftwareBitmap (растровое изображение в памяти) из декодера
    let software_bitmap = decoder.GetSoftwareBitmapAsync()?.await?;

    // Создаём OCR-движок для языка пользователя (на основе региональных настроек)
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    // Распознаём текст на изображении
    let result = engine.RecognizeAsync(&software_bitmap)?.await?;
    // Извлекаем распознанный текст как String
    Ok(result.Text()?.to_string())
}

/// Выбирает слово из распознанного текста (упрощённо — первое слово).
/// Параметры _click_x, _click_y пока не используются, но могут понадобиться
/// для определения конкретного слова по координатам клика.
pub fn get_word_at_position(ocr_text: &str, _click_x: f32, _click_y: f32) -> Option<String> {
    // Разбиваем текст по пробелам и возвращаем первое слово
    ocr_text.split_whitespace().next().map(|s| s.to_string())
}
