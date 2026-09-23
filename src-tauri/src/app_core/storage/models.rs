use serde::{Deserialize, Serialize};

use super::error::{StorageError, StorageResult};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedWord {
    pub id: i64,
    pub word: String,
    pub word_translation: String,
    pub meaning: String,
    pub sentence: String,
    pub sentence_translation: String,
    pub part_of_speech: String,
    pub topic: String,
    pub synonyms: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewWord {
    pub word: String,
    pub word_translation: String,

    #[serde(default)]
    pub meaning: String,

    #[serde(default)]
    pub sentence: String,

    #[serde(default)]
    pub sentence_translation: String,

    #[serde(default)]
    pub part_of_speech: String,

    #[serde(default)]
    pub topic: String,

    #[serde(default)]
    pub synonyms: Vec<String>,
}

impl NewWord {
    pub fn validated(mut self) -> StorageResult<Self> {
        self.word = self.word.trim().to_string();
        self.word_translation = self.word_translation.trim().to_string();

        if self.word.is_empty() {
            return Err(StorageError::Validation(
                "word must not be empty".to_string(),
            ));
        }

        Ok(self)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWord {
    pub word_translation: String,
    pub meaning: String,
}

impl UpdateWord {
    pub fn validated(mut self) -> StorageResult<Self> {
        self.word_translation = self.word_translation.trim().to_string();
        self.meaning = self.meaning.trim().to_string();

        if self.word_translation.is_empty() {
            return Err(StorageError::Validation(
                "translation must not be empty".to_string(),
            ));
        }

        Ok(self)
    }
}
