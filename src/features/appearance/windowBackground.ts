// The native window's background follows the page's. Windows fills a resized window with that colour before the webview has
// drawn at the new size, so a colour that differs from the app's shows as a strip at the moving edge (PLAN §9, 6q-0 decisions
// 8 and 10; docs/spikes/S12-browser-pane.md finding 23).
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../../lib/window/appWindow";

/** A colour as sRGB bytes. */
export type Rgb = [number, number, number];

/**
 * Returns a function that reads the background and sends it when it differs from the last one sent. A send that fails is
 * forgotten, so the next call sends that colour again.
 */
export function backgroundSync(read: () => Rgb | null, send: (rgb: Rgb) => Promise<void>): () => void {
  let last: Rgb | null = null;
  return () => {
    const rgb = read();
    if (!rgb || (last && rgb.every((channel, i) => channel === last?.[i]))) return;
    last = rgb;
    send(rgb).catch(() => {
      if (last === rgb) last = null;
    });
  };
}

/** A fill no theme uses: if it is still there after the page's colour is set, the canvas refused that colour. */
const UNSET = "#010203";

/**
 * The page's background as bytes, or null when it can't be read. The tokens are `oklch()`, which a computed style gives back
 * as written, so a 1x1 canvas turns it into sRGB. In forced colours the page is drawn on the system's `Canvas` colour, which
 * a computed style doesn't show.
 */
export function readPageBackground(doc: Document = document): Rgb | null {
  const view = doc.defaultView;
  if (!view) return null;
  const css = view.matchMedia(FORCED_COLORS).matches ? "Canvas" : view.getComputedStyle(doc.body).backgroundColor;
  const canvas = doc.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.fillStyle = UNSET;
  ctx.fillStyle = css;
  if (ctx.fillStyle === UNSET) return null;
  ctx.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0, a = 0] = ctx.getImageData(0, 0, 1, 1).data;
  // A see-through page background has no one colour to give the window.
  return a === 255 ? [r, g, b] : null;
}

const FORCED_COLORS = "(forced-colors: active)";
/** What can change the page's background: the system's scheme and forced colours here, the Appearance theme on the root. */
const QUERIES = ["(prefers-color-scheme: dark)", FORCED_COLORS];

/** Keeps the window's background equal to the page's, now and whenever the theme changes. Returns the stop function. */
export function watchWindowBackground(doc: Document = document): () => void {
  const view = doc.defaultView;
  if (!inTauri() || !view) return () => {};
  const sync = backgroundSync(
    () => readPageBackground(doc),
    (rgb) => invoke("set_window_background", { rgb }),
  );
  const queries = QUERIES.map((q) => view.matchMedia(q));
  for (const q of queries) q.addEventListener("change", sync);
  const themeWatch = new MutationObserver(sync);
  themeWatch.observe(doc.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  sync();
  return () => {
    themeWatch.disconnect();
    for (const q of queries) q.removeEventListener("change", sync);
  };
}
