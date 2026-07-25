import { useEffect, useState } from "react";
import "./App.css";
import flag from "./assets/Flag_of_Russia.png";

interface TranslationData {
  sentence: string;
  word: string;
  sentence_translation: string;
  word_translation: string;
  synonyms: string[];
  part_of_speech: string;
  topic: string;
}

const bookmark = (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    height="34px"
    viewBox="0 -960 960 960"
    width="34px"
    fill="#F0A06C"
  >
    <path d="M200-120v-640q0-33 23.5-56.5T280-840h400q33 0 56.5 23.5T760-760v640L480-240 200-120Zm80-122 200-86 200 86v-518H280v518Zm0-518h400-400Z" />
  </svg>
);

const bookmark_check = (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    height="34px"
    viewBox="0 -960 960 960"
    width="34px"
    fill="#f46200"
  >
    <path d="M200-120v-640q0-33 23.5-56.5T280-840h400q33 0 56.5 23.5T760-760v640L480-240 200-120Z" />
  </svg>
);

function App() {
  const [check, setCheck] = useState(false);
  const [translationData, setTranslationData] = useState<TranslationData>();

  useEffect(() => {
    const handler = () => {
      const data = (window as any).__translationData;

      console.log(data);
      setTranslationData(data);
    };
    window.addEventListener("translationDataReady", handler);
    return () => window.removeEventListener("translationDataReady", handler);
  }, []);

  if (!translationData) {
    return null;
  }

  return (
    <div className="App">
      <button className="bookmark" onClick={() => setCheck(!check)}>
        {check ? bookmark : bookmark_check}
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
}

export default App;
