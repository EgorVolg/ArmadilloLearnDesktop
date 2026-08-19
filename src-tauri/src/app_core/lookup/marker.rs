use super::image::Image;

pub fn draw_click_marker(image: &mut Image, x: i32, y: i32) {
    const RADIUS: i32 = 14;

    const YELLOW_R: f32 = 255.0;
    const YELLOW_G: f32 = 220.0;
    const YELLOW_B: f32 = 0.0;

    const ALPHA: f32 = 0.4;

    let width = image.width as i32;
    let height = image.height as i32;

    for dy in -RADIUS..=RADIUS {
        for dx in -RADIUS..=RADIUS {
            if dx * dx + dy * dy > RADIUS * RADIUS {
                continue;
            }

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

    for offset in -3..=3 {
        set_pixel(image, x + offset, y, 255, 220, 0);

        set_pixel(image, x, y + offset, 255, 220, 0);
    }
}

fn set_pixel(image: &mut Image, x: i32, y: i32, r: u8, g: u8, b: u8) {
    if x < 0 || y < 0 || x >= (image.width as i32) || y >= (image.height as i32) {
        return;
    }

    let index = ((y as usize) * (image.width as usize) + (x as usize)) * 3;

    image.data[index] = r;
    image.data[index + 1] = g;
    image.data[index + 2] = b;
}
