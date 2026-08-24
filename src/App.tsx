import { useEffect } from "react";
import "./App.css";
import { MainApp, OverlayApp } from "./windows";
import { enable } from "@tauri-apps/plugin-autostart";

function App() {
  useEffect(() => {
    enable().catch(console.error);
  }, []);
  const hash = window.location.hash;

  switch (hash) {
    case "#/overlay":
      return <OverlayApp />;

    case "#/main":
      return <MainApp />;

    default:
      return <MainApp />;
  }
}

export default App;
