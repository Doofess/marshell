import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/accents.css";
import "./styles/scrollbars.css";
import "./styles/base.css";
import "./styles/app.css";
import { syncPageHidden } from "./design/visibility";

syncPageHidden(document);

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
