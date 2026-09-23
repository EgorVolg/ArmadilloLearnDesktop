use rusqlite::Connection;

use super::error::{StorageError, StorageResult};

const MIGRATIONS: &[&str] = &[
    // Версия 1
    r#"
    CREATE TABLE words (
        id                   INTEGER PRIMARY KEY,
        word                 TEXT    NOT NULL UNIQUE COLLATE NOCASE,
        word_translation     TEXT    NOT NULL,
        meaning              TEXT    NOT NULL DEFAULT '',
        sentence             TEXT    NOT NULL DEFAULT '',
        sentence_translation TEXT    NOT NULL DEFAULT '',
        part_of_speech       TEXT    NOT NULL DEFAULT '',
        topic                TEXT    NOT NULL DEFAULT '',
        synonyms             TEXT    NOT NULL DEFAULT '[]',
        created_at           INTEGER NOT NULL,
        updated_at           INTEGER NOT NULL
    );

    CREATE INDEX idx_words_created_at ON words (created_at DESC);
    "#,
];

pub fn migrate(conn: &mut Connection) -> StorageResult<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let latest = MIGRATIONS.len() as i64;

    if current > latest {
        return Err(StorageError::UnsupportedSchema {
            found: current,
            supported: latest,
        });
    }

    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
    }

    Ok(())
}
