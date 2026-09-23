use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("word with id {0} not found")]
    NotFound(i64),

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("database schema v{found} is newer than supported v{supported}")]
    UnsupportedSchema { found: i64, supported: i64 },

    #[error("database lock is poisoned")]
    LockPoisoned,
}

impl StorageError {
    /// Короткий код ошибки для фронтенда.
    /// По нему TypeScript понимает, что именно случилось.
    pub fn code(&self) -> &'static str {
        match self {
            StorageError::Database(_) => "database",
            StorageError::Io(_) => "io",
            StorageError::NotFound(_) => "not_found",
            StorageError::Validation(_) => "validation",
            StorageError::UnsupportedSchema { .. } => "unsupported_schema",
            StorageError::LockPoisoned => "lock_poisoned",
        }
    }
}

/// Tauri отправляет ошибку команды во фронтенд как JSON.
/// Превращаем любую StorageError в объект { "code": ..., "message": ... }.
impl Serialize for StorageError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_struct("StorageError", 2)?;
        object.serialize_field("code", self.code())?;
        object.serialize_field("message", &self.to_string())?;
        object.end()
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_becomes_json_with_code_and_message() {
        let json = serde_json::to_value(StorageError::NotFound(7)).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "code": "not_found",
                "message": "word with id 7 not found"
            })
        );
    }

    #[test]
    fn validation_becomes_json_with_code_and_message() {
        let error = StorageError::Validation("word must not be empty".to_string());
        let json = serde_json::to_value(error).unwrap();

        assert_eq!(json["code"], "validation");
        assert_eq!(json["message"], "invalid input: word must not be empty");
    }
}
