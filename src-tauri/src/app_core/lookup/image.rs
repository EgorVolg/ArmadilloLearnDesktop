pub struct Image {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}
pub fn draw_ocr_highlight(image: &mut Image, min_x: f32, min_y: f32, max_x: f32, max_y: f32) {
    // Небольшой отступ вокруг текста.
    const PADDING_X: i32 = 4;
    const PADDING_Y: i32 = 3;

    // Жёлтый цвет подсветки.
    const YELLOW_R: u8 = 255;
    const YELLOW_G: u8 = 220;
    const YELLOW_B: u8 = 40;

    // Прозрачность подсветки.
    // 0   = оригинальный фон
    // 255 = полностью жёлтый
    const ALPHA: u16 = 110;

    let left = (min_x.floor() as i32 - PADDING_X).max(0);
    let top = (min_y.floor() as i32 - PADDING_Y).max(0);

    let right = (max_x.ceil() as i32 + PADDING_X).min(image.width as i32);

    let bottom = (max_y.ceil() as i32 + PADDING_Y).min(image.height as i32);

    if left >= right || top >= bottom {
        return;
    }

    for y in top..bottom {
        for x in left..right {
            let index = ((y as usize) * (image.width as usize) + (x as usize)) * 3;

            let old_r = image.data[index] as u16;
            let old_g = image.data[index + 1] as u16;
            let old_b = image.data[index + 2] as u16;

            image.data[index] = ((old_r * (255 - ALPHA) + (YELLOW_R as u16) * ALPHA) / 255) as u8;

            image.data[index + 1] =
                ((old_g * (255 - ALPHA) + (YELLOW_G as u16) * ALPHA) / 255) as u8;

            image.data[index + 2] =
                ((old_b * (255 - ALPHA) + (YELLOW_B as u16) * ALPHA) / 255) as u8;
        }
    }
}

pub fn crop_around_point(
    source: &Image,
    click_x: i32,
    click_y: i32,
    crop_width: u32,
    crop_height: u32,
) -> Image {
    let mut result = Image {
        width: crop_width,
        height: crop_height,
        data: vec![0; (crop_width as usize) * (crop_height as usize) * 3],
    };

    let source_width = source.width as i32;
    let source_height = source.height as i32;

    let crop_width_i32 = crop_width as i32;
    let crop_height_i32 = crop_height as i32;

    let source_left = click_x - crop_width_i32 / 2;
    let source_top = click_y - crop_height_i32 / 2;

    for dst_y in 0..crop_height_i32 {
        for dst_x in 0..crop_width_i32 {
            let src_x = source_left + dst_x;
            let src_y = source_top + dst_y;

            if src_x < 0 || src_y < 0 || src_x >= source_width || src_y >= source_height {
                continue;
            }

            let src_index = ((src_y as usize) * (source.width as usize) + (src_x as usize)) * 3;

            let dst_index = ((dst_y as usize) * (crop_width as usize) + (dst_x as usize)) * 3;

            result.data[dst_index] = source.data[src_index];

            result.data[dst_index + 1] = source.data[src_index + 1];

            result.data[dst_index + 2] = source.data[src_index + 2];
        }
    }

    result
}

pub fn upscale_nearest(source: &Image, scale: u32) -> Image {
    if scale <= 1 {
        return Image {
            width: source.width,
            height: source.height,
            data: source.data.clone(),
        };
    }

    let width = source.width.saturating_mul(scale);
    let height = source.height.saturating_mul(scale);

    let mut data = vec![0; (width as usize) * (height as usize) * 3];

    for y in 0..height {
        let source_y = y / scale;

        for x in 0..width {
            let source_x = x / scale;

            let source_index =
                ((source_y as usize) * (source.width as usize) + (source_x as usize)) * 3;

            let destination_index = ((y as usize) * (width as usize) + (x as usize)) * 3;

            data[destination_index] = source.data[source_index];

            data[destination_index + 1] = source.data[source_index + 1];

            data[destination_index + 2] = source.data[source_index + 2];
        }
    }

    Image {
        width,
        height,
        data,
    }
}

pub fn encode_png(image: &Image) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;

    let mut output = Vec::new();

    let encoder = image::codecs::png::PngEncoder::new(&mut output);

    encoder
        .write_image(
            &image.data,
            image.width,
            image.height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("Failed to encode PNG: {error}"))?;

    Ok(output)
}
