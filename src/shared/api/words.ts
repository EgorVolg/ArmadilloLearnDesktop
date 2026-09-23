import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { NewWord, SavedWord, StorageError, UpdateWord } from "../types";

// Каждая функция вызывает одну Rust-команду из commands.rs.
// Имена аргументов (entry, search, id, word, changes) должны
// совпадать с именами параметров в Rust.

export function saveWord(entry: NewWord): Promise<SavedWord> {
  return invoke<SavedWord>("save_word", { entry });
}

export function listWords(search: string): Promise<SavedWord[]> {
  return invoke<SavedWord[]>("list_words", { search });
}

export function getWord(id: number): Promise<SavedWord> {
  return invoke<SavedWord>("get_word", { id });
}

/** Возвращает null, если слова нет в базе. */
export function findWord(word: string): Promise<SavedWord | null> {
  return invoke<SavedWord | null>("find_word", { word });
}

export function updateWord(
  id: number,
  changes: UpdateWord,
): Promise<SavedWord> {
  return invoke<SavedWord>("update_word", { id, changes });
}

export function deleteWord(id: number): Promise<void> {
  return invoke<void>("delete_word", { id });
}

/** Подписка на событие "words-changed" (слово сохранили/изменили/удалили). */
export function onWordsChanged(callback: () => void): Promise<UnlistenFn> {
  return listen("words-changed", () => callback());
}

/** Проверяет, что ошибка пришла из нашей базы ({ code, message }). */
export function isStorageError(error: unknown): error is StorageError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  );
}

/** Превращает любую ошибку в текст для человека. */
export function errorText(error: unknown): string {
  if (isStorageError(error)) {
    switch (error.code) {
      case "not_found":
        return "Слово не найдено — возможно, его уже удалили.";
      case "validation":
        return `Неверные данные: ${error.message}`;
      default:
        return `Ошибка базы: ${error.message}`;
    }
  }

  return String(error);
}
