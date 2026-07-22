import { useState } from "react";
import "./App.css";
import flag from "./assets/Flag_of_Russia.png";

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
          <section className="word">Capability</section>
        </article>

        <article>
          <section className="container-header">
            <h1>Перевод</h1>
            <hr />
          </section>

          <section className="tags">
            <span>Noun</span>
            <span>Programming</span>
          </section>

          <section className="word">
            <img src={flag} alt="russian language" />
            Возможность
          </section>

          <section className="definition">
            The power or ability to do something, or a specific feature or
            function that a system has The power or ability to do something, or
            a specific feature or function that a system has
          </section>

          <section className="synonyms">
            Synonyms:<span>&nbsp;ability, capacity, functionality</span>
          </section>
        </article>
      </main>
    </div>
  );
}

export default App;
