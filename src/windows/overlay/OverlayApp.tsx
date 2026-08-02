import { useEffect, useState } from "react";
import "./OverlayApp.css";
import { Bookmark, BookmarkCheck } from "../../assets/Bookmark";
import flag from "../../assets/Flag_of_Russia.png";
import { TranslationDataType } from "../../shared";

export const OverlayApp = () => {
  const [check, setCheck] = useState(false);
  const [translationData, setTranslationData] = useState<TranslationDataType>({
    sentence: "",
    word: "",
    sentence_translation: "",
    word_translation: "",
    synonyms: [],
    part_of_speech: "",
    topic: "",
  });

  useEffect(() => {
    const handler = () => {
      const data = (window as any).__translationData;

      console.log(data);
      setTranslationData(data);
    };
    window.addEventListener("translationDataReady", handler);
    return () => window.removeEventListener("translationDataReady", handler);
  }, []);

  return (
    <div className="overlay-app">
      <button className="bookmark" onClick={() => setCheck(!check)}>
        {check ? <Bookmark /> : <BookmarkCheck />}
      </button>
      <main className="container">
        <article>
          <div className="container-header">
            <h1>Слово</h1>
            <hr />
          </div>
          <section className="word">{translationData.word}</section>
        </article>

        <article>
          <section className="container-header">
            <h1>Перевод</h1>
            <hr />
          </section>

          <section className="tags">
            <span>{translationData.part_of_speech}</span>
            <span>{translationData.topic}</span>
          </section>

          <section className="word">
            <img src={flag} alt="russian language" />
            {translationData.word_translation}
          </section>

          <section className="definition">
            {translationData.sentence_translation}
          </section>

          <section className="synonyms">
            Synonyms:<span>&nbsp;{translationData.synonyms.join(", ")}</span>
          </section>
        </article>
      </main>
    </div>
  );
};
