use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::{
    error::{StorageError, StorageResult},
    migrations,
    models::{NewWord, SavedWord, UpdateWord},
};

const WORD_COLUMNS: &str = "id, word, word_translation, meaning, sentence, \
     sentence_translation, part_of_speech, topic, synonyms, created_at, updated_at";

pub struct WordRepository {
    conn: Mutex<Connection>,
}

impl WordRepository {
    pub fn open(path: &Path) -> StorageResult<Self> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }

        let conn = Connection::open(path)?;

        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;

        Self::from_connection(conn)
    }

    fn from_connection(mut conn: Connection) -> StorageResult<Self> {
        migrations::migrate(&mut conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> StorageResult<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| StorageError::LockPoisoned)
    }

    pub fn create(&self, new_word: NewWord) -> StorageResult<SavedWord> {
        let new_word = new_word.validated()?;

        let synonyms = serde_json::to_string(&new_word.synonyms)
            .map_err(|error| StorageError::Validation(error.to_string()))?;

        let now = now_ms();

        let sql = format!(
            "INSERT INTO words (
                 word, word_translation, meaning, sentence, sentence_translation,
                 part_of_speech, topic, synonyms, created_at, updated_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT (word) DO UPDATE SET
                 word_translation     = excluded.word_translation,
                 meaning              = excluded.meaning,
                 sentence             = excluded.sentence,
                 sentence_translation = excluded.sentence_translation,
                 part_of_speech       = excluded.part_of_speech,
                 topic                = excluded.topic,
                 synonyms             = excluded.synonyms,
                 updated_at           = excluded.updated_at
             RETURNING {WORD_COLUMNS}"
        );

        let conn = self.conn()?;

        let saved = conn.query_row(
            &sql,
            params![
                new_word.word,
                new_word.word_translation,
                new_word.meaning,
                new_word.sentence,
                new_word.sentence_translation,
                new_word.part_of_speech,
                new_word.topic,
                synonyms,
                now,
            ],
            row_to_word,
        )?;

        Ok(saved)
    }
    pub fn get(&self, id: i64) -> StorageResult<SavedWord> {
        let sql = format!("SELECT {WORD_COLUMNS} FROM words WHERE id = ?1");

        let conn = self.conn()?;

        let found = conn.query_row(&sql, params![id], row_to_word).optional()?;

        match found {
            Some(word) => Ok(word),
            None => Err(StorageError::NotFound(id)),
        }
    }

    pub fn find_by_word(&self, word: &str) -> StorageResult<Option<SavedWord>> {
        let word = word.trim();

        let sql = format!("SELECT {WORD_COLUMNS} FROM words WHERE word = ?1");

        let conn = self.conn()?;

        let found = conn
            .query_row(&sql, params![word], row_to_word)
            .optional()?;

        Ok(found)
    }

    pub fn list(&self, search: &str) -> StorageResult<Vec<SavedWord>> {
        let pattern = format!("%{}%", escape_like(search.trim()));

        let sql = format!(
            "SELECT {WORD_COLUMNS} FROM words
             WHERE word LIKE ?1 ESCAPE '\\'
                OR word_translation LIKE ?1 ESCAPE '\\'
             ORDER BY created_at DESC"
        );

        let conn = self.conn()?;

        let mut statement = conn.prepare(&sql)?;

        let rows = statement.query_map(params![pattern], row_to_word)?;

        let mut words = Vec::new();

        for row in rows {
            words.push(row?);
        }

        Ok(words)
    }

    pub fn update(&self, id: i64, changes: UpdateWord) -> StorageResult<SavedWord> {
        let changes = changes.validated()?;

        let sql = format!(
            "UPDATE words
             SET word_translation = ?1,
                 meaning          = ?2,
                 updated_at       = ?3
             WHERE id = ?4
             RETURNING {WORD_COLUMNS}"
        );

        let conn = self.conn()?;

        let updated = conn
            .query_row(
                &sql,
                params![changes.word_translation, changes.meaning, now_ms(), id],
                row_to_word,
            )
            .optional()?;

        match updated {
            Some(word) => Ok(word),
            None => Err(StorageError::NotFound(id)),
        }
    }

    pub fn delete(&self, id: i64) -> StorageResult<()> {
        let conn = self.conn()?;

        let deleted_count = conn.execute("DELETE FROM words WHERE id = ?1", params![id])?;

        if deleted_count == 0 {
            return Err(StorageError::NotFound(id));
        }

        Ok(())
    }
}

fn row_to_word(row: &Row<'_>) -> rusqlite::Result<SavedWord> {
    let synonyms_text: String = row.get(8)?;

    let synonyms: Vec<String> = serde_json::from_str(&synonyms_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(SavedWord {
        id: row.get(0)?,
        word: row.get(1)?,
        word_translation: row.get(2)?,
        meaning: row.get(3)?,
        sentence: row.get(4)?,
        sentence_translation: row.get(5)?,
        part_of_speech: row.get(6)?,
        topic: row.get(7)?,
        synonyms,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn escape_like(text: &str) -> String {
    let mut result = String::new();

    for character in text.chars() {
        if character == '%' || character == '_' || character == '\\' {
            result.push('\\');
        }
        result.push(character);
    }

    result
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_word(word: &str, translation: &str) -> NewWord {
        NewWord {
            word: word.to_string(),
            word_translation: translation.to_string(),
            meaning: String::new(),
            sentence: String::new(),
            sentence_translation: String::new(),
            part_of_speech: String::new(),
            topic: String::new(),
            synonyms: vec!["jog".to_string()],
        }
    }

    #[test]
    fn create_saves_word() {
        let repo = WordRepository::open_in_memory().unwrap();

        let saved = repo.create(new_word("run", "бежать")).unwrap();

        assert!(saved.id > 0);
        assert_eq!(saved.word, "run");
        assert_eq!(saved.word_translation, "бежать");
        assert_eq!(saved.synonyms, vec!["jog".to_string()]);
    }

    #[test]
    fn create_trims_spaces() {
        let repo = WordRepository::open_in_memory().unwrap();

        let saved = repo.create(new_word("  run  ", "бежать")).unwrap();

        assert_eq!(saved.word, "run");
    }

    #[test]
    fn create_rejects_empty_word() {
        let repo = WordRepository::open_in_memory().unwrap();

        let result = repo.create(new_word("   ", "пусто"));

        assert!(matches!(result, Err(StorageError::Validation(_))));
    }

    #[test]
    fn create_same_word_twice_keeps_one_record() {
        let repo = WordRepository::open_in_memory().unwrap();

        let first = repo.create(new_word("run", "бежать")).unwrap();
        let second = repo.create(new_word("RUN", "управлять")).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.word_translation, "управлять");
    }
    #[test]
    fn get_returns_word_by_id() {
        let repo = WordRepository::open_in_memory().unwrap();
        let saved = repo.create(new_word("run", "бежать")).unwrap();

        let found = repo.get(saved.id).unwrap();

        assert_eq!(found, saved);
    }

    #[test]
    fn get_missing_id_returns_not_found() {
        let repo = WordRepository::open_in_memory().unwrap();

        let result = repo.get(999);

        assert!(matches!(result, Err(StorageError::NotFound(999))));
    }

    #[test]
    fn find_by_word_ignores_case() {
        let repo = WordRepository::open_in_memory().unwrap();
        repo.create(new_word("Armadillo", "броненосец")).unwrap();

        let found = repo.find_by_word("armadillo").unwrap();
        let missing = repo.find_by_word("zebra").unwrap();

        assert!(found.is_some());
        assert!(missing.is_none());
    }

    #[test]
    fn list_returns_all_when_search_is_empty() {
        let repo = WordRepository::open_in_memory().unwrap();
        repo.create(new_word("run", "бежать")).unwrap();
        repo.create(new_word("walk", "идти")).unwrap();

        let words = repo.list("").unwrap();

        assert_eq!(words.len(), 2);
    }

    #[test]
    fn list_searches_in_word_and_translation() {
        let repo = WordRepository::open_in_memory().unwrap();
        repo.create(new_word("run", "бежать")).unwrap();
        repo.create(new_word("walk", "идти")).unwrap();
        repo.create(new_word("runner", "бегун")).unwrap();

        assert_eq!(repo.list("run").unwrap().len(), 2);
        assert_eq!(repo.list("идти").unwrap()[0].word, "walk");
    }

    #[test]
    fn list_treats_percent_as_normal_text() {
        let repo = WordRepository::open_in_memory().unwrap();
        repo.create(new_word("run", "бежать")).unwrap();

        let words = repo.list("%").unwrap();

        assert_eq!(words.len(), 0);
    }

    #[test]
    fn update_changes_translation_and_meaning() {
        let repo = WordRepository::open_in_memory().unwrap();
        let saved = repo.create(new_word("run", "бежать")).unwrap();

        let changes = UpdateWord {
            word_translation: "руководить".to_string(),
            meaning: "to manage".to_string(),
        };
        let updated = repo.update(saved.id, changes).unwrap();

        assert_eq!(updated.word_translation, "руководить");
        assert_eq!(updated.meaning, "to manage");
        assert_eq!(updated.word, "run");
    }

    #[test]
    fn update_missing_id_returns_not_found() {
        let repo = WordRepository::open_in_memory().unwrap();

        let changes = UpdateWord {
            word_translation: "что-то".to_string(),
            meaning: String::new(),
        };
        let result = repo.update(999, changes);

        assert!(matches!(result, Err(StorageError::NotFound(999))));
    }

    #[test]
    fn delete_removes_word() {
        let repo = WordRepository::open_in_memory().unwrap();
        let saved = repo.create(new_word("run", "бежать")).unwrap();

        repo.delete(saved.id).unwrap();

        assert_eq!(repo.list("").unwrap().len(), 0);
    }

    #[test]
    fn delete_missing_id_returns_not_found() {
        let repo = WordRepository::open_in_memory().unwrap();

        let result = repo.delete(999);

        assert!(matches!(result, Err(StorageError::NotFound(999))));
    }
}
