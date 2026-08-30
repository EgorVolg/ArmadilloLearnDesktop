export interface TranslationDataType {
  meaning: string;
  word: string;
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

type ThemeName = "dark" | "light" | "glass-light" | "glass-dark";

type ThemeValues = `${ThemeName}`;

export interface Theme {
  name: ThemeValues;
  ico: string;
}
