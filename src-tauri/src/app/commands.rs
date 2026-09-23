use tauri::{AppHandle, Emitter, State};

use crate::app_core::storage::{
    error::StorageResult,
    models::{NewWord, SavedWord, UpdateWord},
    repository::WordRepository,
};

/// Имя события, которое получают все окна после изменения словаря.
const WORDS_CHANGED: &str = "words-changed";

/// Сообщает всем окнам: «список слов изменился, обновитесь».
/// Если событие не отправилось, слово всё равно уже сохранено,
/// поэтому просто пишем ошибку в консоль.
fn notify_words_changed(app: &AppHandle) {
    if let Err(error) = app.emit(WORDS_CHANGED, ()) {
        eprintln!("Failed to emit {WORDS_CHANGED}: {error}");
    }
}

/// Сохраняет слово. Если такое слово уже есть — обновляет его.
#[tauri::command]
pub fn save_word(
    app: AppHandle,
    repository: State<'_, WordRepository>,
    entry: NewWord,
) -> StorageResult<SavedWord> {
    let saved = repository.create(entry)?;

    notify_words_changed(&app);

    Ok(saved)
}

/// Возвращает слова. Пустая строка поиска — все слова.
#[tauri::command]
pub fn list_words(
    repository: State<'_, WordRepository>,
    search: String,
) -> StorageResult<Vec<SavedWord>> {
    repository.list(&search)
}

/// Возвращает одно слово по id.
#[tauri::command]
pub fn get_word(repository: State<'_, WordRepository>, id: i64) -> StorageResult<SavedWord> {
    repository.get(id)
}

/// Ищет слово по тексту (без учёта регистра).
/// Возвращает null во фронтенд, если слова нет.
#[tauri::command]
pub fn find_word(
    repository: State<'_, WordRepository>,
    word: String,
) -> StorageResult<Option<SavedWord>> {
    repository.find_by_word(&word)
}

/// Меняет перевод и значение слова.
#[tauri::command]
pub fn update_word(
    app: AppHandle,
    repository: State<'_, WordRepository>,
    id: i64,
    changes: UpdateWord,
) -> StorageResult<SavedWord> {
    let updated = repository.update(id, changes)?;

    notify_words_changed(&app);

    Ok(updated)
}

/// Удаляет слово по id.
#[tauri::command]
pub fn delete_word(
    app: AppHandle,
    repository: State<'_, WordRepository>,
    id: i64,
) -> StorageResult<()> {
    repository.delete(id)?;

    notify_words_changed(&app);

    Ok(())
}
