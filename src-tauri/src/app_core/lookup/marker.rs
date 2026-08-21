use super::image::Image;

pub fn draw_click_marker(image: &mut Image, x: i32, y: i32) {
    // Горизонтальная полоска-подчёркивание. Её центр совпадает с точкой клика,
    // а текст для перевода модель ищет сразу под центром полоски.
    const HALF_WIDTH: i32 = 20;
    const THICKNESS: i32 = 30;

    const YELLOW_R: f32 = 255.0;
    const YELLOW_G: f32 = 220.0;
    const YELLOW_B: f32 = 0.0;

    const ALPHA: f32 = 0.4;

    let width = image.width as i32;
    let height = image.height as i32;

    // Единая полупрозрачная полоска-подчёркивание.
    for dy in -(THICKNESS / 2)..=(THICKNESS / 2) {
        for dx in -HALF_WIDTH..=HALF_WIDTH {
            let px = x + dx;
            let py = y + dy;

            if px < 0 || py < 0 || px >= width || py >= height {
                continue;
            }

            let index = ((py as usize) * (image.width as usize) + (px as usize)) * 3;

            let r = image.data[index] as f32;
            let g = image.data[index + 1] as f32;
            let b = image.data[index + 2] as f32;

            image.data[index] = (r * (1.0 - ALPHA) + YELLOW_R * ALPHA).round() as u8;
            image.data[index + 1] = (g * (1.0 - ALPHA) + YELLOW_G * ALPHA).round() as u8;
            image.data[index + 2] = (b * (1.0 - ALPHA) + YELLOW_B * ALPHA).round() as u8;
        }
    }
}
