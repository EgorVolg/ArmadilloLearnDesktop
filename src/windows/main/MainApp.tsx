import { useState } from "react";
import "./MainApp.css";
import { setTheme, Theme } from "../../shared";
import { Header } from "./components/header/Header";
import { Navbar } from "./components/navbar/Navbar";
import { MainContent } from "./components/main-content/MainContent";

export const MainApp = () => {
  const [selected] = useState(false);
  const [selectedTheme, setSelectedTheme] = useState<Theme>("light");
  const [openedLink, setOpenedLink] = useState(1);
  const [value, setValue] = useState("");

  function openLink(id: number) {
    setOpenedLink(id);
  }

  function SwitchTheme() {
    setSelectedTheme(selectedTheme === "light" ? "dark" : "light");
    setTheme(selectedTheme);
  }

  return (
    <div className="main-app">
      <Header
        openedLink={openedLink}
        onOpenLink={openLink}
        onSwitchTheme={SwitchTheme}
      />
      <Navbar value={value} onChange={setValue} />
      <MainContent selected={selected} />
    </div>
  );
};
