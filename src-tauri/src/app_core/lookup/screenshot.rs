use screenshots::Screen;

use super::image::Image;

pub fn capture_screen() -> Result<Image, String> {
    println!("Getting screens...");

    let screens = Screen::all().map_err(|error| { format!("Failed to get screens: {error}") })?;

    let screen = screens.first().ok_or_else(|| "No screens found".to_string())?;

    println!("Capturing first screen...");

    let screenshot = screen
        .capture()
        .map_err(|error| { format!("Failed to capture screen: {error}") })?;

    let width = screenshot.width();
    let height = screenshot.height();

    println!("Screenshot captured: {}x{}", width, height);

    let pixels = screenshot.as_raw();

    let expected_len = (width as usize) * (height as usize) * 4;

    if pixels.len() != expected_len {
        return Err(format!("Invalid frame size: expected {}, got {}", expected_len, pixels.len()));
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

    Ok(Image {
        width,
        height,
        data: rgb,
    })
}
