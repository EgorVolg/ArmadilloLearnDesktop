use image::{DynamicImage, ImageBuffer, Rgba};
use screenshots::Screen;
use std::io::Cursor;
use windows::{
    core::*,
    Foundation::Rect,
    Graphics::Imaging::BitmapDecoder,
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};

/// Информация о распознанном слове с его позицией на изображении
#[derive(Debug, Clone)]
pub struct OcrWordInfo {
    pub text: String,
    pub rect: Rect, // bounding box относительно всего изображения (source coords)
}

/// Захватывает область экрана вокруг точки (x, y) и возвращает PNG-байты.
pub fn capture_area(x: i32, y: i32, width: u32, height: u32) -> Option<Vec<u8>> {
    let screen = Screen::from_point(x, y).ok()?;
    let img = screen
        .capture_area(x - width as i32 / 2, y - height as i32 / 2, width, height)
        .ok()?;

    let (w, h) = (img.width(), img.height());
    let raw_data = img.into_raw();

    let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(w, h, raw_data).expect("raw data should match dimensions");

    let dynamic = DynamicImage::ImageRgba8(img_buffer);
    let mut png_bytes = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .ok()?;

    Some(png_bytes)
}

/// Принимает PNG-байты и возвращает распознанный текст через Windows OCR.
pub async fn ocr_from_png(png_bytes: Vec<u8>) -> Result<String> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(&png_bytes)?;
    writer.StoreAsync()?.await?;
    writer.FlushAsync()?.await?;
    stream.Seek(0)?;

    let decoder =
        BitmapDecoder::CreateWithIdAsync(BitmapDecoder::PngDecoderId()?, &stream)?.await?;
    let software_bitmap = decoder.GetSoftwareBitmapAsync()?.await?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let result = engine.RecognizeAsync(&software_bitmap)?.await?;
    Ok(result.Text()?.to_string())
}

/// Распознаёт текст и возвращает слова с bounding boxes.
/// Каждый `OcrWordInfo::rect` — координаты относительно левого верхнего угла
/// захваченного изображения.
pub async fn ocr_from_png_with_words(png_bytes: Vec<u8>) -> Result<Vec<OcrWordInfo>> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(&png_bytes)?;
    writer.StoreAsync()?.await?;
    writer.FlushAsync()?.await?;
    stream.Seek(0)?;

    let decoder =
        BitmapDecoder::CreateWithIdAsync(BitmapDecoder::PngDecoderId()?, &stream)?.await?;
    let software_bitmap = decoder.GetSoftwareBitmapAsync()?.await?;

    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let result = engine.RecognizeAsync(&software_bitmap)?.await?;

    let mut words = Vec::new();
    let lines = result.Lines()?;
    for line in lines {
        let line_words = line.Words()?;
        for w in line_words {
            words.push(OcrWordInfo {
                text: w.Text()?.to_string(),
                rect: w.BoundingRect()?,
            });
        }
    }

    Ok(words)
}

/// Определяет слово, ближайшее к точке (click_x, click_y).
/// Координаты клика (click_x, click_y) — относительно левого верхнего угла
/// захваченного изображения (т.е. от OCR_CENTER_X, OCR_CENTER_Y).
pub fn get_word_at_position(
    words: &[OcrWordInfo],
    click_x: f32,
    click_y: f32,
) -> Option<String> {
    if words.is_empty() {
        return None;
    }

    // Находим слово, bounding box которого содержит точку клика
    // Если такого нет — возвращаем ближайшее по центру
    for w in words {
        let r = &w.rect;
        if click_x >= r.X && click_x <= r.X + r.Width
            && click_y >= r.Y && click_y <= r.Y + r.Height
        {
            return Some(w.text.clone());
        }
    }

    // Если ни один bounding box не содержит точку — берём слово с ближайшим центром
    let closest = words
        .iter()
        .min_by(|a, b| {
            let a_cx = a.rect.X + a.rect.Width / 2.0;
            let a_cy = a.rect.Y + a.rect.Height / 2.0;
            let b_cx = b.rect.X + b.rect.Width / 2.0;
            let b_cy = b.rect.Y + b.rect.Height / 2.0;
            let a_dist = (click_x - a_cx).powi(2) + (click_y - a_cy).powi(2);
            let b_dist = (click_x - b_cx).powi(2) + (click_y - b_cy).powi(2);
            a_dist.partial_cmp(&b_dist).unwrap_or(std::cmp::Ordering::Equal)
        })?;

    Some(closest.text.clone())
}