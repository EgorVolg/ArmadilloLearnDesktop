use super::image::Image;

/// Подсвечивает OCR-текст прямо на screenshot,
/// который будет отправлен Vision AI.
///
/// bbox:
/// (min_x, min_y, max_x, max_y)
pub fn draw_ocr_highlight(image: &mut Image, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
    const PADDING: i32 = 2;

    // Жёлтая подсветка.
    const YELLOW_R: f32 = 255.0;
    const YELLOW_G: f32 = 220.0;
    const YELLOW_B: f32 = 0.0;

    // Полупрозрачность.
    const ALPHA: f32 = 0.35;

    let image_width = image.width as i32;
    let image_height = image.height as i32;

    let start_x = min_x.floor() as i32 - PADDING;
    let start_y = min_y.floor() as i32 - PADDING;
    let end_x = max_x.ceil() as i32 + PADDING;
    let end_y = max_y.ceil() as i32 + PADDING;

    for y in start_y..=end_y {
        if y < 0 || y >= image_height {
            continue;
        }

        for x in start_x..=end_x {
            if x < 0 || x >= image_width {
                continue;
            }

            let index = ((y as usize) * (image.width as usize) + x as usize) * 3;

            let r = image.data[index] as f32;
            let g = image.data[index + 1] as f32;
            let b = image.data[index + 2] as f32;

            image.data[index] = (r * (1.0 - ALPHA) + YELLOW_R * ALPHA).round() as u8;

            image.data[index + 1] = (g * (1.0 - ALPHA) + YELLOW_G * ALPHA).round() as u8;

            image.data[index + 2] = (b * (1.0 - ALPHA) + YELLOW_B * ALPHA).round() as u8;
        }
    }
}
