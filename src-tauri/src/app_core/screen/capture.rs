use crate::app_core::lookup::image::Image;
use screenshots::Screen;

pub struct CapturedScreen {
    pub image: Image,

    /// Левая верхняя точка монитора
    /// в глобальной системе координат desktop.
    pub origin_x: i32,
    pub origin_y: i32,

    /// Координаты клика относительно
    /// верхнего левого угла screenshot.
    pub click_x: i32,
    pub click_y: i32,
}

pub fn capture_screen(click_x: i32, click_y: i32) -> Result<CapturedScreen, String> {
    let screen = Screen::from_point(click_x, click_y)
        .map_err(|error| format!("Failed to find screen at ({click_x}, {click_y}): {error}"))?;

    let display = screen.display_info;

    let local_click_x = click_x - display.x;
    let local_click_y = click_y - display.y;

    let screenshot = screen
        .capture()
        .map_err(|error| format!("Failed to capture screen: {error}"))?;

    let width = screenshot.width();
    let height = screenshot.height();

    let pixels = screenshot.as_raw();

    let expected_len = (width as usize) * (height as usize) * 4;

    if pixels.len() != expected_len {
        return Err(format!(
            "Invalid frame size: expected {}, got {}",
            expected_len,
            pixels.len()
        ));
    }

    let mut rgb = Vec::with_capacity((width as usize) * (height as usize) * 3);

    for pixel in pixels.chunks_exact(4) {
        let b = pixel[0];
        let g = pixel[1];
        let r = pixel[2];

        rgb.push(r);
        rgb.push(g);
        rgb.push(b);
    }

    Ok(CapturedScreen {
        image: Image {
            width,
            height,
            data: rgb,
        },
        origin_x: display.x,
        origin_y: display.y,
        click_x: local_click_x,
        click_y: local_click_y,
    })
}
