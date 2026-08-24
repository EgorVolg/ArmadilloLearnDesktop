import { useEffect, useState } from "react";
import "./OverlayApp.css";
import { Bookmark, BookmarkCheck } from "../../assets/Bookmark";
import flag from "../../assets/Flag_of_Russia.png";
import { LookupError, TranslationDataType } from "../../shared";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export const OverlayApp = () => {
  const [check, setCheck] = useState(false);
  const [error, setError] = useState<LookupError>();
  const [translationData, setTranslationData] = useState<TranslationDataType>({
    sentence: "",
    word: "",
    sentence_translation: "",
    word_translation: "",
    synonyms: [],
    part_of_speech: "",
    topic: "",
  });

  async function openMainWindow() {
    await invoke("open_main_window");
  }

  const renderSentenceWithHighlight = () => {
    const target = translationData.word.toLowerCase();
    let highlighted = false;

    return translationData.sentence.split(/(\s+)/).map((part, index) => {
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
                padding: "0 2px",
                borderRadius: "4px",
                fontWeight: "bold",
                fontSize: "18px",
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

  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [
      listen<TranslationDataType>("lookup-result", (event) => {
        console.log("Translation data:", event.payload);

        setError(undefined);
        setTranslationData(event.payload);
      }),

      listen<LookupError>("lookup-error", (event) => {
        console.error("Lookup error!!!!!!:", event.payload.message);

        setError(event.payload);
      }),
    ];

    return () => {
      unlisteners.forEach((unlisten) => {
        unlisten.then((fn) => fn());
      });
    };
  }, []);

  return (
    <div className="overlay-app">
      <button className="bookmark" onClick={() => setCheck(!check)}>
        {check ? <Bookmark /> : <BookmarkCheck />}
      </button>
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
            <button onClick={openMainWindow}>+</button>
            <article>
              <div className="container-header">
                <h1>Слово</h1>
                <hr />
              </div>
              <section className="word">
                {renderSentenceWithHighlight()}
              </section>
            </article>

            <article className="translation-article">
              <section className="container-header">
                <h1>Перевод</h1>
                <hr />
              </section>

              <section className="word">
                <img src={flag} alt="russian language" />
                {translationData.word_translation}
              </section>
            </article>

            <article className="translation-content">
              <section className="tags">
                <span>{translationData.part_of_speech}</span>
                <span>{translationData.topic}</span>
              </section>

              <section className="definition">
                {translationData.sentence_translation}
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
