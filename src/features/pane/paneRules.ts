import type { DraftComment } from "../../generated/DraftComment";
import type { PaneView } from "../../generated/PaneView";
import { VIEWPORTS } from "../browser/viewport";
import { PANELS, isPanelId, panelOf } from "./panels";

const VIEWPORT_IDS: readonly string[] = VIEWPORTS.map((v) => v.id);

/** A folder of a saved tree, as `tree_folder_ok` in the core: relative to the root, "/" separated, no empty, "." or ".." part. */
export function treeFolderOk(f: string): boolean {
  return f.length > 0 && f.length <= 4096 && !/[\\:]/.test(f) && !f.startsWith("/") && f.split("/").every((c) => c !== "" && c !== "." && c !== "..");
}

// Control characters as Rust's `char::is_control` has them (C0, DEL and C1).
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/g;
const CONTROL_BUT_TAB = /[\u0000-\u0008\u000a-\u001f\u007f-\u009f]/;
const CONTROL_BUT_TAB_LF = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;
const points = (s: string) => [...s].length;
const lowerHex = (s: string) => /^[0-9a-f]{1,16}$/.test(s);

/** A saved Changes comment the core keeps, as `comment_problem` in the core finds nothing wrong with it (unique ids are the caller's). */
export function draftCommentOk(c: DraftComment): boolean {
  return (
    /^[a-z0-9-]{1,64}$/.test(c.id) &&
    treeFolderOk(c.path) &&
    c.start >= 1 &&
    c.end >= c.start &&
    c.rows >= 1 &&
    c.rows <= 100_000 &&
    lowerHex(c.first) &&
    lowerHex(c.hash) &&
    c.quote.length <= 3 &&
    c.quote.every((q) => points(q) <= 201 && !CONTROL_BUT_TAB.test(q)) &&
    points(c.text) >= 1 &&
    points(c.text) <= 2000 &&
    !CONTROL_BUT_TAB_LF.test(c.text)
  );
}

/** A compared base as the core keeps it: no control character, at most 200 characters (a longer one is cut, ending in "…"). */
export function draftBase(base: string): string {
  const chars = [...base.replace(CONTROL, "")];
  return chars.length <= 200 ? chars.join("") : `${chars.slice(0, 199).join("")}…`;
}

/**
 * A test helper: what the core accepts of a pane, in TypeScript. Returns the name of the first field that breaks a rule, or null.
 * While a session has tabs its `active` names one of them (a Files tab when Files has any, else a Browser tab); `active_browser` is
 * a property present only while Browser has a tab, and names one of them; `panels` has `shown` non-empty in the stack's order,
 * `filled` null or one of two shown, and weights that are whole numbers from 1 to 100000 under panel ids.
 */
export function paneProblem(pane: PaneView): string | null {
  if (pane.tree?.width != null && (pane.tree.width < 160 || pane.tree.width > 2048)) return "tree.width";
  for (const st of Object.values(pane.sessions)) {
    if (st.tree && (st.tree.expanded.length > 500 || !st.tree.expanded.every(treeFolderOk))) return "tree.expanded";
    const panels = st.tabs.map(panelOf);
    if (st.tabs.length > 32) return "tabs";
    if (new Set(st.tabs.map((t) => t.id)).size !== st.tabs.length) return "tabs";
    const files = st.tabs.filter((_, i) => panels[i] === "files");
    const browser = st.tabs.filter((_, i) => panels[i] === "browser");
    if (st.tabs.some((t) => t.viewport !== undefined && !VIEWPORT_IDS.includes(t.viewport))) return "viewport";
    if ("active_browser" in st && typeof st.active_browser !== "string") return "active_browser";
    if (st.tabs.length > 0) {
      const named = st.tabs.find((t) => t.id === st.active);
      if (!named) return "active";
      if (files.length > 0 && !files.includes(named)) return "active";
    } else if (st.active !== null) {
      return "active";
    }
    if (browser.length === 0) {
      if ("active_browser" in st) return "active_browser";
    } else if (!browser.some((t) => t.id === st.active_browser)) {
      return "active_browser";
    }
  }
  const p = pane.panels;
  if (p !== undefined) {
    if (!Array.isArray(p.shown) || p.shown.length === 0) return "panels.shown";
    const order = PANELS.filter((id) => p.shown.includes(id));
    if (order.length !== p.shown.length || order.some((id, i) => p.shown[i] !== id)) return "panels.shown";
    if (p.filled !== null && (typeof p.filled !== "string" || !p.shown.includes(p.filled) || p.shown.length < 2)) return "panels.filled";
    for (const [key, value] of Object.entries(p.weights)) {
      if (!isPanelId(key)) return "panels.weights";
      if (typeof value !== "number" || !Number.isInteger(value) || value < 1 || value > 100_000) return "panels.weights";
    }
  }
  return null;
}
