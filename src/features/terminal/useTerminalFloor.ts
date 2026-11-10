import { useEffect, useMemo, useState } from "react";
import { terminalFloorWidth } from "./columns";
import { useScreenScale } from "./useScreenScale";

/** The part of `document.fonts` this needs. */
type FontSet = {
  ready: Promise<unknown>;
  addEventListener(type: "loadingdone", listener: () => void): void;
  removeEventListener(type: "loadingdone", listener: () => void): void;
};

/**
 * Calls `tell` once when the page's fonts are ready and again each time a font finishes loading, until the returned function is
 * called. A glyph measured before its font has loaded is the fallback font's, so whoever measured asks again.
 */
export function onFontsLoaded(fonts: FontSet | undefined, tell: () => void): () => void {
  if (!fonts) return () => {};
  let live = true;
  const loaded = () => {
    if (live) tell();
  };
  fonts.addEventListener("loadingdone", loaded);
  fonts.ready.then(loaded, () => {});
  return () => {
    live = false;
    fonts.removeEventListener("loadingdone", loaded);
  };
}

/**
 * The least width of a terminal that still fits `cols` columns in this font (columns.ts), worked out again when the screen's
 * scale changes and when a font finishes loading: the first measure at the app's start can come before the terminal's font has
 * loaded, and it then counted a wider fallback glyph, which put the side pane's 80-column stop far too early.
 */
export function useTerminalFloor(cols: number, font: { family: string; size: number }): number {
  const scale = useScreenScale();
  const [loads, setLoads] = useState(0);
  useEffect(() => onFontsLoaded(typeof document === "undefined" ? undefined : document.fonts, () => setLoads((n) => n + 1)), []);
  // `loads` is a dependency so the canvas measures again with the loaded font.
  return useMemo(() => terminalFloorWidth(cols, font, undefined, scale), [cols, font, scale, loads]);
}
