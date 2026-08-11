/// Прямоугольная область изображения.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// Координата X левого верхнего угла.
    pub x: i32,

    /// Координата Y левого верхнего угла.
    pub y: i32,

    /// Ширина области.
    pub width: u32,

    /// Высота области.
    pub height: u32,
}

impl Region {
    /// Создаёт новую прямоугольную область.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
