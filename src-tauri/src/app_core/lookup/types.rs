use serde::{ Deserialize, Serialize };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub sentence: String,
    pub word: String,
    pub sentence_translation: String,
    pub word_translation: String,
    pub synonyms: Vec<String>,
    pub part_of_speech: String,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupError {
    pub code: String,
    pub message: String,
}
