pub struct Crop {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub struct RecognitionResult {
    pub text: String,
    pub confidence: f32,
}
