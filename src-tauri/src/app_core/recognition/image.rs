/// RGB-изображение.
/// Каждый пиксель занимает 3 байта:
/// [R, G, B, R, G, B, ...]
///
/// Формат:
/// - 8 бит на канал;
/// - 3 канала;
/// - без alpha;
/// - непрерывный буфер.
/// 
#[derive(Debug, Clone)]
pub struct Image { 
    pub width: u32, 
    pub height: u32,
    /// RGB-пиксели.
    pub data: Vec<u8>,
}

impl Image {
    /// Создаёт изображение из готового RGB-буфера.
    ///
    /// Проверяем, что размер буфера соответствует
    /// width × height × 3.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, String> {
        let expected_len = width as usize * height as usize * 3;

        if data.len() != expected_len {
            return Err(format!(
                "Invalid RGB buffer size: expected {}, got {}",
                expected_len,
                data.len()
            ));
        }

        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Возвращает количество байт на один пиксель.
    pub const fn bytes_per_pixel() -> usize {
        3
    }

    /// Возвращает размер изображения в байтах.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Проверяет, пустое ли изображение.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
