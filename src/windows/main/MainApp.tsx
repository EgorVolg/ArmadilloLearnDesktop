import { useState } from "react";
import "./MainApp.css";
import { Header } from "./components/header/Header";
import { Navbar } from "./components/navbar/Navbar";
import { MainContent } from "./components/main-content/MainContent";

export const MainApp = () => {
  const [selected] = useState(false);

  const [openedLink, setOpenedLink] = useState(1);
  const [value, setValue] = useState("");

  function openLink(id: number) {
    setOpenedLink(id);
  }

  return (
    <div className="main-app">
      <Header openedLink={openedLink} onOpenLink={openLink} />
      <Navbar value={value} onChange={setValue} />
      <MainContent selected={selected} />
    </div>
  );
};
