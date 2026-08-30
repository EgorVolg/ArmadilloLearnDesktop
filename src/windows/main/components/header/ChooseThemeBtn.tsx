import { useState } from "react";
import moon from "../../../../assets/Moon.png";
import sun from "../../../../assets/Sun.png";
import light from "../../../../assets/light-gems.png";
import dark from "../../../../assets/rainbow-gems.png";
import { Theme } from "../../../../shared";
import "./ChooseThemeBtn.css";

export const themes: Theme[] = [
  { name: "light", ico: sun },
  { name: "dark", ico: moon },
  { name: "glass-light", ico: light },
  { name: "glass-dark", ico: dark },
];

export const ChooseThemeBtn = () => {
  const [selectedTheme, setSelectedTheme] = useState(themes[0]);

  function setTheme(theme: Theme): void {
    setSelectedTheme(theme);
    document.documentElement.setAttribute("data-theme", theme.name);
  }

  return (
    <div className="main-app__header-nav-theme-switcher-container">
      <button className="main-app__header-nav-theme-switcher-button">
        <img
          src={selectedTheme.ico}
          alt="Theme switcher"
          className="main-app__header-nav-theme-switcher-button-image"
        />
      </button>
      <ul className="main-app__header-nav-theme-switcher-list">
        {themes.map((theme: Theme) => (
          <li onClick={() => setTheme(theme)}>
            <button className="bbtn">
              <img
                src={theme.ico}
                alt="Theme switcher"
                className="main-app__header-nav-theme-switcher-button-image"
              />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
};
