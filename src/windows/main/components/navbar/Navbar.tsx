import { useState } from "react";
import { Close, Language, Search } from "../../../../assets";
import "./Navbar.css";

interface NavbarProps {
  value: string;
  onChange: (value: string) => void;
}

const languages = ["EN", "RU", "FR", "UK", "BG"];

export const Navbar = ({ value, onChange }: NavbarProps) => {
  const [showLangList, setShowLangList] = useState(false);
  const [selectedLanguage, setSelectedLanguage] = useState("EN");

  function closePopUp(lang: string) {
    setSelectedLanguage(lang);
    setShowLangList(false);
  }

  return (
    <nav className="main-app__nav">
      <div className="main-app__nav-search">
        <div className="main-app__nav-search-input_container">
          <input
            type="text"
            placeholder="Search words"
            value={value}
            onChange={(e) => onChange(e.target.value)}
          />

          {value.length > 0 ? (
            <button
              className="main-app__nav-search-input_btn"
              onClick={() => onChange("")}
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

        <div className="main-app_nav-language-button-content">
          <button
            className="main-app__nav-language-button"
            onClick={() => setShowLangList(!showLangList)}
          >
            <Language width={20} height={20} color="#F5E4D8" />
            {selectedLanguage}
          </button>

          <ul
            className={`main-app__nav-language-list ${!showLangList && "none"}`}
          >
            {languages.map((lang, i) => (
              <li key={i} className="main-app__nav-language-list-li">
                <button onClick={() => closePopUp(lang)}>{lang}</button>
              </li>
            ))}
          </ul>
        </div>
      </div>
      {/* <div className="main-app__nav-buttons">
        <button className="main-app__nav-select-all-button">Select All</button>
        <button className="main-app__nav-delete-button">
          <Trash width={17} height={17} color="#F5E4D8" />
          Delete
        </button>
      </div> */}
    </nav>
  );
};
