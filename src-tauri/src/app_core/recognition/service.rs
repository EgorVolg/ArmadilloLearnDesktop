use crate::app_core::recognition::{
    paddle::PaddleRecognizer,
    types::{Crop, RecognitionResult},
};

pub struct RecognitionService {
    recognizer: PaddleRecognizer,
}

impl RecognitionService {
    pub fn recognize(&self, crop: Crop) -> RecognitionResult {
        self.recognizer.recognize(crop)
    }
}
