use super::types::TextRegion;

/// Прямоугольная область на экране.
///
/// Используется для описания зоны захвата изображения
/// при распознавании текста.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// Координата X левого верхнего угла области (в пикселях).    \
    /// Высота области (в пикселях).
    /// Координата Y левого верхнего угла области (в пикселях).
    /// Ширина области (в пикселях).
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    /// Создаёт новую область с заданными координатами и размерами.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Проверяет, находится ли точка с координатами `(x, y)` внутри области.
    /// Границы области считаются включительными слева и сверху
    /// и исключительными справа и снизу.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + self.width as i32
            && y < self.y + self.height as i32
    }
    /// Используется для преобразования результата распознавания
    /// текста в область экрана.
    pub fn from_text_region(text: &TextRegion) -> Self {
        Self {
            x: text.x as i32,
            y: text.y as i32,
            width: text.width,
            height: text.height,
        }
    }
}
