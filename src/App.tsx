import "./App.css";
import { MainApp, OverlayApp } from "./windows";

function App() {
  const hash = window.location.hash;

  switch (hash) {
    case "#/overlay":
      return <OverlayApp />;

    case "#/main":
      return <MainApp />;

    default:
      return <OverlayApp />;
  }
}

export default App;
