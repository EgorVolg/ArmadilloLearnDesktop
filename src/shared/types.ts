export interface TranslationDataType {
  meaning: string;
  word: string;
  sentence: string;
  sentence_translation: string;
  word_translation: string;
  synonyms: string[];
  part_of_speech: string;
  topic: string;
}

export type LookupError = {
  code: string;
  message: string;
};

/** Слово из базы (то, что возвращает Rust: SavedWord). */
export interface SavedWord {
  id: number;
  word: string;
  word_translation: string;
  meaning: string;
  sentence: string;
  sentence_translation: string;
  part_of_speech: string;
  topic: string;
  synonyms: string[];
  /** Время в миллисекундах (как Date.now()). */
  created_at: number;
  updated_at: number;
}

/** Новое слово для сохранения (Rust: NewWord). */
export interface NewWord {
  word: string;
  word_translation: string;
  meaning?: string;
  sentence?: string;
  sentence_translation?: string;
  part_of_speech?: string;
  topic?: string;
  synonyms?: string[];
}

/** Что можно поменять у слова (Rust: UpdateWord). */
export interface UpdateWord {
  word_translation: string;
  meaning: string;
}

/** Ошибка базы (Rust: StorageError после Serialize). */
export interface StorageError {
  code:
    | "database"
    | "io"
    | "not_found"
    | "validation"
    | "unsupported_schema"
    | "lock_poisoned";
  message: string;
}

type ThemeName = "dark" | "light" | "glass-light" | "glass-dark";

type ThemeValues = `${ThemeName}`;

export interface Theme {
  name: ThemeValues;
  ico: string;
}
