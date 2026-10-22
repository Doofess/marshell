import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/accents.css";
import "./styles/appearance.css";
import "./styles/scrollbars.css";
import "./styles/base.css";
import "./styles/motion.css";
import "./styles/app.css";
import { syncPageHidden } from "./design/visibility";
import { markDesktopHost } from "./lib/window/appWindow";

syncPageHidden(document);
markDesktopHost();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
