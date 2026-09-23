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

pub type StorageResult<T> = Result<T, StorageError>;
