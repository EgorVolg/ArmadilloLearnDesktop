#[derive(Debug, Clone)]
pub struct TextRegion {
    //
    // координаты области
    //
    pub x: i32,
    pub y: i32,

    //
    // размер
    //
    pub width: u32,
    pub height: u32,

    //
    // текст если есть
    //
    pub text: Option<String>,
}
