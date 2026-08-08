//////////// Есть один важный момент
//////////// data: Vec<u8> не говорит нам, сколько байт приходится на пиксель.
//////////// Для Paddle это важно.
//////////// Например:
//////////// RGBA:
//////////// R G B A
//////////// ↓ ↓ ↓ ↓
//////////// 4 bytes / pixel
////////////
//////////// или:
////////////
//////////// RGB:
//////////// R G B
//////////// ↓ ↓ ↓
//////////// 3 bytes / pixel

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, format: PixelFormat, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            format,
            data,
        }
    }

    pub fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            format: PixelFormat::Rgba8,
            data: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.data.is_empty()
    }
}
