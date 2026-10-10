import type { ReactNode } from "react";
import "./ThemePair.css";

/** Renders the same content in a dark and a light panel; the tokens use light-dark(), which follows color-scheme. */
export function ThemePair({ children, label, stack = false }: { children: ReactNode; label?: string; stack?: boolean }) {
  return (
    <section className="theme-pair" data-stack={stack || undefined} aria-label={label}>
      <div className="theme-pair__panel" style={{ colorScheme: "dark" }} data-panel="dark">
        <p className="theme-pair__caption">Dark</p>
        {children}
      </div>
      <div className="theme-pair__panel" style={{ colorScheme: "light" }} data-panel="light">
        <p className="theme-pair__caption">Light</p>
        {children}
      </div>
    </section>
  );
}
