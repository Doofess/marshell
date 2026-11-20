// The native window's theme follows the Appearance theme. WebView2 draws what the page can't style in the window's theme: the
// right-click menu above all. Left alone the window keeps the system's theme, so a light Marshell on a dark Windows showed a
// dark menu.
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../../lib/window/appWindow";

/** The window's theme: one Appearance names, or the system's. */
export type WindowTheme = "light" | "dark" | "system";

/** The window's theme for the root's `data-theme`, which is absent when Appearance follows the system. */
export function windowThemeOf(attr: string | null): WindowTheme {
  return attr === "light" || attr === "dark" ? attr : "system";
}

/**
 * Returns a function that reads the theme and sends it when it differs from the last one sent. A send that fails is
 * forgotten, so the next call sends that theme again.
 */
export function themeSync(read: () => WindowTheme, send: (theme: WindowTheme) => Promise<void>): () => void {
  let last: { theme: WindowTheme } | null = null;
  return () => {
    const theme = read();
    if (last?.theme === theme) return;
    const sent = { theme };
    last = sent;
    send(theme).catch(() => {
      if (last === sent) last = null;
    });
  };
}

/** Keeps the window's theme equal to the Appearance theme, now and whenever it changes. Returns the stop function. */
export function watchWindowTheme(doc: Document = document): () => void {
  if (!inTauri()) return () => {};
  const root = doc.documentElement;
  const sync = themeSync(
    () => windowThemeOf(root.getAttribute("data-theme")),
    (theme) =>
      invoke<void>("set_window_theme", { theme }).catch((error: unknown) => {
        // Not silent: until the next theme change the right-click menu keeps the old theme.
        console.error("Marshell: the window's theme couldn't be set.", error);
        throw error;
      }),
  );
  const themeWatch = new MutationObserver(sync);
  themeWatch.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
  sync();
  return () => themeWatch.disconnect();
}
