import { useState } from "react";
import { Language, Search, Trash, Close } from "../../assets";
import "./MainApp.css";
import Logo from "../../assets/Logo.png";
import Switcher from "../../assets/potted-plant-icon.png";
import Test from "../../assets/test.png";

const language = "EN";

const headerNavItems = [
  {
    id: 1,
    name: "Words",
    href: "#",
  },
  {
    id: 2,
    name: "Translations & Dictionaries",
    href: "#",
  },
  {
    id: 3,
    name: "Settings",
    href: "#",
  },
];

const wordsData = [
  {
    id: 0,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 1,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 2,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 3,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 4,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 5,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 6,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 7,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 8,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 9,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 10,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 11,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 12,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 13,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
  {
    id: 14,
    name: "Word 1",
    image: "https://via.placeholder.com/150",
    translations: ["Translation 1", "Translation 1", "Translation 1"],
    description:
      "Lorem ipsum dolor sit amet consectetur adipisicing elit. Recusandae deleniti cupiditate est ea, quis temporibus fuga maiores aspernatur atque, accusamus itaque neque nostrum deserunt! Ad perspiciatis suscipit dicta impedit nisi!",
  },
];

export const MainApp = () => {
  const [selected, setSelected] = useState(false);
  const [openedLink, setOpenedLink] = useState(headerNavItems[0].id);
  const [value, setValue] = useState("");
  function openLink(id: number) {
    setOpenedLink(id);
  }

  return (
    <div className="main-app">
      <article className="main-app__header">
        <nav className="main-app__header-nav">
          <div className="main-app__header-logo">
            <img
              src={Logo}
              alt="logo"
              className="main-app__header-logo-image"
            />
            <h3 className="main-app__header-logo-text">Armadillo Learn</h3>
          </div>

          <ul className="main-app__header-nav-list">
            {headerNavItems.map((item) => (
              <li
                key={item.id}
                className={`${openedLink === item.id ? "main-app__header-nav-item-active" : "main-app__header-nav-item"}`}
                onClick={() => openLink(item.id)}
              >
                <a href={item.href} className="main-app__header-nav-link">
                  {item.name}
                </a>
              </li>
            ))}
          </ul>

          <button className="main-app__header-nav-theme-switcher-button">
            <img
              src={Switcher}
              alt="Theme switcher"
              className="main-app__header-nav-theme-switcher-button-image"
            />
          </button>
        </nav>
      </article>

      <nav className="main-app__nav">
        <div className="main-app__nav-search">
          <div className="main-app__nav-search-input_container">
            <input
              type="text"
              placeholder="Search words"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />

            {value.length > 0 ? (
              <button
                className="main-app__nav-search-input_btn"
                onClick={() => setValue("")}
              >
                <Close />
              </button>
            ) : (
              <Search
                className="main-app__nav-search-input_icon"
                width={18}
                height={18}
              />
            )}
          </div>

          <button className="main-app__nav-language-button">
            <Language width={20} height={20} color="#F5E4D8" />
            {language}
          </button>
        </div>

        <div className="main-app__nav-buttons">
          <button className="main-app__nav-select-all-button">
            Select All
          </button>
          <button className="main-app__nav-delete-button">
            <Trash width={17} height={17} color="#F5E4D8" />
            Delete
          </button>
        </div>
      </nav>

      <main className="main-app__main">
        <aside className="main-app__sidebar">
          <div className="main-app__sidebar-list-container">
            <div className="main-app__sidebar-title">Today</div>
            <ul className="main-app__sidebar-list">
              {wordsData.map((word) => (
                <li key={word.id} className="main-app__sidebar-item">
                  {selected && (
                    <input
                      type="checkbox"
                      className="main-app__sidebar-item-checkbox"
                      checked={selected}
                    />
                  )}
                  <div className="main-app__sidebar-item-content">
                    <p className="main-app__sidebar-item-content-title">
                      {word.name + "-" + word.id}
                    </p>

                    <div className="main-app__sidebar-item-content-tags">
                      {word.translations.map((el, index) => (
                        <p
                          className="main-app__sidebar-item-content-tags-tag"
                          key={index}
                        >
                          {word.translations.length - 1 === index
                            ? el
                            : `${el}\u00A0·\u00A0`}
                        </p>
                      ))}
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </aside>

        <article className="main-app__article">
          <section className="main-app__article-section">
            <h1 className="main-app__article-section-title">
              {wordsData[0].name}
            </h1>
            <hr className="main-app__article-section-hr" />
            <p className="main-app__article-section-subtitle">
              {wordsData[0].translations[0]}
            </p>
          </section>
          <section className="main-app__article-section-content">
            <img
              src={Test}
              alt={wordsData[0].name}
              className="main-app__article-section-image"
            />
          </section>
          <section className="main-app__article-section-content">
            <div className="main-app__article-section-content-translations">
              A reply or answer to a request, message, or signal.
            </div>
          </section>
        </article>
      </main>
    </div>
  );
};
