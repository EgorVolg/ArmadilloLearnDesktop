#[derive(Debug, Clone)]
pub struct TextRegion {
    pub text: String,

    pub x: u32,
    pub y: u32,

    pub width: u32,
    pub height: u32,

    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub regions: Vec<TextRegion>,
}
