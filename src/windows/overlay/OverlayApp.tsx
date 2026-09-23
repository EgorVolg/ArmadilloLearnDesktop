import { useEffect, useRef, useState } from "react";
import "./OverlayApp.css";
import flag from "../../assets/Flag_of_Russia.png";
import {
  deleteWord,
  errorText,
  findWord,
  LookupError,
  onWordsChanged,
  saveWord,
  TranslationDataType,
} from "../../shared";
import { listen } from "@tauri-apps/api/event";
import { Bookmark, BookmarkCheck, Language, Translate } from "../../assets";

export const OverlayApp = () => {
  // id слова в базе или null, если слово не сохранено.
  const [savedId, setSavedId] = useState<number | null>(null);
  // true, пока идёт сохранение/удаление (защита от двойного клика).
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<LookupError>();

  const [translationData, setTranslationData] = useState<TranslationDataType>({
    meaning: "",
    word: "",
    sentence: "",
    sentence_translation: "",
    word_translation: "",
    synonyms: [],
    part_of_speech: "",
    topic: "",
  });

  const renderSentenceWithHighlight = () => {
    const target = translationData.word_translation.toLowerCase();
    let highlighted = false;

    return translationData.sentence_translation
      .split(/(\s+)/)
      .map((part, index) => {
        const core = part.match(/[\p{L}\p{N}]+/u)?.[0] ?? "";

        if (!highlighted && core !== "" && core.toLowerCase() === target) {
          highlighted = true;
          const coreIndex = part.indexOf(core);

          return (
            <span key={index}>
              {part.slice(0, coreIndex)}
              <span
                style={{
                  backgroundColor: "rgb(255, 221, 0)",
                  color: "black",
                  padding: "0 4px",
                  borderRadius: "5px",
                  fontWeight: "bold",
                  fontSize: "14px",
                }}
              >
                {core}
              </span>
              {part.slice(coreIndex + core.length)}
            </span>
          );
        }

        return <span key={index}>{part}</span>;
      });
  };
  const currentWordRef = useRef("");

  // Проверяет, сохранено ли текущее слово в базе.
  const refreshSaved = async () => {
    const word = currentWordRef.current;

    if (word === "") {
      setSavedId(null);
      return;
    }

    try {
      const found = await findWord(word);

      // Пока ждали ответ, пользователь мог кликнуть другое слово.
      // Тогда этот ответ уже устарел — игнорируем его.
      if (currentWordRef.current === word) {
        setSavedId(found ? found.id : null);
      }
    } catch (e) {
      console.error("findWord failed:", e);
    }
  };

  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [
      listen<TranslationDataType>("lookup-result", (event) => {
        console.log("Translation data:", event.payload);

        setError(undefined);
        setTranslationData(event.payload);

        currentWordRef.current = event.payload.word;
        setSavedId(null);
        refreshSaved();
      }),

      listen<LookupError>("lookup-error", (event) => {
        console.error("Lookup error!!!!!!:", event.payload.message);

        currentWordRef.current = "";
        setSavedId(null);
        setError(event.payload);
      }),

      // Слово могли удалить в главном окне — перепроверяем закладку.
      onWordsChanged(() => {
        refreshSaved();
      }),
    ];

    return () => {
      unlisteners.forEach((unlisten) => {
        unlisten.then((fn) => fn());
      });
    };
  }, []);

  // Клик по закладке: не сохранено → сохранить, сохранено → удалить.
  const toggleBookmark = async () => {
    if (busy || translationData.word === "") return;

    setBusy(true);

    try {
      if (savedId !== null) {
        await deleteWord(savedId);
        setSavedId(null);
      } else {
        const saved = await saveWord({
          word: translationData.word,
          word_translation: translationData.word_translation,
          meaning: translationData.meaning,
          sentence: translationData.sentence,
          sentence_translation: translationData.sentence_translation,
          part_of_speech: translationData.part_of_speech,
          topic: translationData.topic,
          synonyms: translationData.synonyms,
        });

        if (currentWordRef.current === translationData.word) {
          setSavedId(saved.id);
        }
      }
    } catch (e) {
      console.error("Bookmark failed:", errorText(e));
      // Показываем правильное состояние из базы.
      refreshSaved();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="overlay-app">
      {!error && translationData.word !== "" && (
        <button
          className="bookmark"
          onClick={toggleBookmark}
          disabled={busy}
          title={
            savedId !== null ? "Удалить из словаря" : "Сохранить в словарь"
          }
        >
          {savedId !== null ? <BookmarkCheck /> : <Bookmark />}
        </button>
      )}

      <main className="container">
        {error ? (
          <article>
            <div className="container-header">
              <h1>Ошибка</h1>
              <hr />
            </div>

            <section
              style={{
                color: "red",
                fontSize: "20px",
                fontWeight: "bold",
                maxWidth: "500px",
              }}
            >
              {error.code}
            </section>
          </article>
        ) : (
          <>
            <article className="article translation">
              <aside className="container-header">
                <h1>Перевод</h1>
                <Translate width={18} height={18} />
                <hr />
              </aside>
              <section className="word">
                {renderSentenceWithHighlight()}
              </section>
            </article>

            <article className="article originaly">
              <aside className="container-header">
                <h1>Слово</h1>
                <Language width={18} height={18} />
                <hr />
              </aside>
              <section className="word">
                <img className="lang-img" src={flag} alt="russian language" />
                {translationData.word_translation}
              </section>

              <section className="tags">
                <span>{translationData.part_of_speech}</span>
                <span>{translationData.topic}</span>
              </section>

              <section className="definition">
                {translationData.meaning}
              </section>

              {translationData.synonyms.length > 0 && (
                <section className="synonyms">
                  Synonyms:
                  <span>&nbsp;{translationData.synonyms.join(", ")}</span>
                </section>
              )}
            </article>
          </>
        )}
      </main>
    </div>
  );
};
