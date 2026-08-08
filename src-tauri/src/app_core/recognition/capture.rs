use screenshots::Screen;

use super::{
    image::{Image, PixelFormat},
    region::Region,
};

use screenshots::Screen;
//////////////////////////!!!!!!!!!'////////////////////////
// !!!!!!!! Используем первый экран из списка. !!!!!!!!!!!!
//////////////////////////!!!!!!!!!'////////////////////////
/// Захватывает указанную область экрана.
///
/// Эта функция отвечает только за получение изображения.
/// Никакого OCR или анализа текста здесь нет.
pub fn capture(region: Region) -> Result<Image, Box<dyn std::error::Error>> {
    // Получаем список всех мониторов, подключённых к системе.
    let screens = Screen::all()?;

    // Пока работаем с первым монитором.
    //
    // Позже здесь нужно будет определить,
    // на каком именно мониторе находится курсор.
    let screen = screens.first().ok_or("No screen found")?;

    // Делаем снимок указанной области экрана.
    let screenshot = screen.capture_area(region.x, region.y, region.width, region.height)?;

    // Библиотека screenshots возвращает изображение
    // с пикселями в формате RGBA.
    //
    // Копируем буфер в наш собственный Image,
    // чтобы остальная часть recognition
    // не зависела от screenshots.
    Ok(Image::new(
        screenshot.width(),
        screenshot.height(),
        PixelFormat::Rgba8,
        screenshot.buffer().to_vec(),
    ))
}
