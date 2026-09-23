import { useState } from "react";
import { ArrowDropDown, ArrowDropUp, Trash } from "../../../../assets";
import Test from "../../../../assets/test.png";
import { errorText, SavedWord, UpdateWord } from "../../../../shared";
import "./MainContent.css";

interface MainContentProps {
  selected: boolean;
  words: SavedWord[];
  selectedWord: SavedWord | null;
  search: string;
  loadError: string;
  onSelect: (id: number) => void;
  onUpdate: (id: number, changes: UpdateWord) => Promise<void>;
  onDelete: (id: number) => Promise<void>;
}

export const MainContent = ({
  selected,
  words,
  selectedWord,
  search,
  loadError,
  onSelect,
  onUpdate,
  onDelete,
}: MainContentProps) => {
  return (
    <main className="main-app__main">
      <aside className="main-app__sidebar">
        <div className="main-app__sidebar-list-container">
          <div className="main-app__sidebar-title">Words ({words.length})</div>

          {loadError !== "" && <p className="words-error">{loadError}</p>}

          {words.length === 0 && loadError === "" && (
            <p className="words-empty">
              {search.trim() !== ""
                ? "Ничего не найдено"
                : "Пока пусто. Сохраните слово закладкой в оверлее."}
            </p>
          )}

          <ul className="main-app__sidebar-list">
            {words.map((word) => (
              <li
                key={word.id}
                className={`main-app__sidebar-item ${
                  selectedWord?.id === word.id
                    ? "main-app__sidebar-item_active"
                    : ""
                }`}
                onClick={() => onSelect(word.id)}
              >
                {selected && (
                  <input
                    type="checkbox"
                    className="main-app__sidebar-item-checkbox"
                    checked={selected}
                    readOnly
                  />
                )}
                <div className="main-app__sidebar-item-content">
                  <p className="main-app__sidebar-item-content-title">
                    {word.word}
                  </p>

                  <div className="main-app__sidebar-item-content-tags">
                    <p className="main-app__sidebar-item-content-tags-tag">
                      {word.word_translation}
                    </p>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </div>
      </aside>

      {selectedWord ? (
        // key: при выборе другого слова WordArticle создаётся заново,
        // и режим редактирования/подтверждения сбрасывается сам.
        <WordArticle
          key={selectedWord.id}
          word={selectedWord}
          onUpdate={onUpdate}
          onDelete={onDelete}
        />
      ) : (
        <article className="main-app__article">
          <p className="words-empty">Выберите слово слева</p>
        </article>
      )}
    </main>
  );
};

interface WordArticleProps {
  word: SavedWord;
  onUpdate: (id: number, changes: UpdateWord) => Promise<void>;
  onDelete: (id: number) => Promise<void>;
}

const WordArticle = ({ word, onUpdate, onDelete }: WordArticleProps) => {
  const [showImage, setShowImage] = useState(false);

  // Режим редактирования и черновик полей.
  const [editing, setEditing] = useState(false);
  const [draftTranslation, setDraftTranslation] = useState(
    word.word_translation,
  );
  const [draftMeaning, setDraftMeaning] = useState(word.meaning);

  // Показан ли вопрос «Удалить?».
  const [confirmDelete, setConfirmDelete] = useState(false);

  // true, пока ждём ответ от базы; текст ошибки для пользователя.
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState("");

  function startEditing() {
    setDraftTranslation(word.word_translation);
    setDraftMeaning(word.meaning);
    setActionError("");
    setEditing(true);
  }

  async function save() {
    setBusy(true);
    setActionError("");

    try {
      await onUpdate(word.id, {
        word_translation: draftTranslation,
        meaning: draftMeaning,
      });
      setEditing(false);
    } catch (e) {
      setActionError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setBusy(true);
    setActionError("");

    try {
      await onDelete(word.id);
      // После удаления этот компонент исчезнет сам
      // (слово пропадёт из списка).
    } catch (e) {
      setActionError(errorText(e));
      setBusy(false);
    }
  }

  return (
    <article className="main-app__article">
      <section className="main-app__article-section">
        <h1 className="main-app__article-section-title">
          {word.word}
          <button
            onClick={() => setShowImage(!showImage)}
            className="main-app__article-section-title_arrow"
          >
            {showImage ? (
              <ArrowDropUp width={15} height={15} />
            ) : (
              <ArrowDropDown width={15} height={15} />
            )}
          </button>
        </h1>

        <hr className="main-app__article-section-hr" />

        {editing ? (
          <input
            className="word-edit__input"
            value={draftTranslation}
            onChange={(e) => setDraftTranslation(e.target.value)}
            placeholder="Перевод"
            disabled={busy}
          />
        ) : (
          <div className="main-app__article-section-subtitles">
            <p className="main-app__article-section-subtitle">
              {word.word_translation}
            </p>
          </div>
        )}

        {(word.part_of_speech !== "" || word.topic !== "") && (
          <div className="word-tags">
            {word.part_of_speech !== "" && <span>{word.part_of_speech}</span>}
            {word.topic !== "" && <span>{word.topic}</span>}
          </div>
        )}
      </section>

      <section className="main-app__article-section_content-translation">
        {editing ? (
          <textarea
            className="word-edit__textarea"
            value={draftMeaning}
            onChange={(e) => setDraftMeaning(e.target.value)}
            placeholder="Значение"
            rows={3}
            disabled={busy}
          />
        ) : (
          word.meaning !== "" && (
            <div className="main-app__article-section-content-translations">
              {word.meaning}
            </div>
          )
        )}

        {word.synonyms.length > 0 && (
          <div className="main-app__article-section-content-translations">
            Synonyms: {word.synonyms.join(", ")}
          </div>
        )}

        {word.sentence !== "" && (
          <div className="main-app__article-section-content-translations-example">
            <p>Example</p>
            <span>{word.sentence}</span>
            {word.sentence_translation !== "" && (
              <span className="word-example__translation">
                {word.sentence_translation}
              </span>
            )}
          </div>
        )}
      </section>

      {actionError !== "" && <p className="words-error">{actionError}</p>}

      <section className="word-actions">
        {editing ? (
          <>
            <button
              className="word-actions__btn"
              onClick={save}
              disabled={busy}
            >
              Сохранить
            </button>
            <button
              className="word-actions__btn"
              onClick={() => setEditing(false)}
              disabled={busy}
            >
              Отмена
            </button>
          </>
        ) : confirmDelete ? (
          <>
            <span className="word-actions__question">
              Удалить «{word.word}»?
            </span>
            <button
              className="word-actions__btn word-actions__btn_danger"
              onClick={remove}
              disabled={busy}
            >
              Да, удалить
            </button>
            <button
              className="word-actions__btn"
              onClick={() => setConfirmDelete(false)}
              disabled={busy}
            >
              Нет
            </button>
          </>
        ) : (
          <>
            <button className="word-actions__btn" onClick={startEditing}>
              Редактировать
            </button>
            <button
              className="word-actions__btn word-actions__btn_danger"
              onClick={() => setConfirmDelete(true)}
            >
              <Trash width={16} height={16} color="#F5E4D8" />
              Удалить
            </button>
          </>
        )}
      </section>

      <section
        className={`${!showImage ? "none" : ""} main-app__article-section_content-img`}
      >
        <img
          src={Test}
          alt={word.word}
          className="main-app__article-section-image"
        />
      </section>
    </article>
  );
};
