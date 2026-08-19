export interface TranslationDataType {
  sentence: string;
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
