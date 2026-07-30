#[derive(Debug, Clone, Copy)]
pub struct CropArea {
    pub x: i32,
    pub y: i32,

    pub width: u32,
    pub height: u32,
}

impl CropArea {
    pub fn around_cursor(cursor_x: i32, cursor_y: i32) -> Self {
        let width = 500;
        let height = 300;

        Self {
            x: cursor_x - (width / 2) as i32,
            y: cursor_y - (height / 2) as i32,

            width,
            height,
        }
    }
}
