import { Close, Language, Search, Trash } from "../../../../assets";
import "./Navbar.css";

const language = "EN";

interface NavbarProps {
  value: string;
  onChange: (value: string) => void;
}

export const Navbar = ({ value, onChange }: NavbarProps) => {
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

        <button className="main-app__nav-language-button">
          <Language width={20} height={20} color="#F5E4D8" />
          {language}
        </button>
      </div>

      <div className="main-app__nav-buttons">
        <button className="main-app__nav-select-all-button">Select All</button>
        <button className="main-app__nav-delete-button">
          <Trash width={17} height={17} color="#F5E4D8" />
          Delete
        </button>
      </div>
    </nav>
  );
};