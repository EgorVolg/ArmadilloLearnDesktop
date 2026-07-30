import "./App.css";
import { MainApp } from "./main_app";
import { OverlayApp } from "./overlay";

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
