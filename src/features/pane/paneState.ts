import type { PaneEntry } from "../../generated/PaneEntry";
import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { Panels } from "../../generated/Panels";
import type { RecentFile } from "../../generated/RecentFile";
import type { SessionTabs } from "../../generated/SessionTabs";
import { BLANK, isPageUrl, pageTitle, sameUrl } from "../browser/address";
import { type PanelId, PANELS, type Stack, type Weights, activeIn, isPanelId, onScreen, panelOf, panelOfPath, panelTabs, stackOf, stackPanels } from "./panels";

export type { PaneEntry };

/** The pane's limits: tabs per session, history entries per tab, recent files saved and shown, and tabs kept loaded. */
export const PANE_LIMITS = { tabs: 32, history: 50, recent: 200, shownRecent: 10, kept: 4 } as const;

/** The narrowest and widest saved width of the pane, in pixels. */
const WIDTH_MIN = 320;
const WIDTH_MAX = 4096;

export type View = "rendered" | "source";
/** Where a PDF is showing: its page from 1, and how far down it, as a fraction of the page's height from 0 to 1. */
export type Place = { page: number; top: number };
export type OpenTarget = { path: string; line?: number; col?: number; view?: View; place?: Place };
export type PaneAction =
  /** A new tab, or its existing tab; shows the pane. */
  | { type: "open"; session: string; project: string; target: OpenTarget }
  /** A link inside a tab: pushes its history. */
  | { type: "navigate"; session: string; paneTab: string; project: string; target: OpenTarget }
  | { type: "back" | "forward"; session: string; paneTab: string }
  | { type: "setView"; session: string; paneTab: string; view: View }
  | { type: "place"; session: string; paneTab: string; place: Place }
  | { type: "select"; session: string; paneTab: string }
  /** The next or previous tab, wrapping. */
  | { type: "step"; session: string; panel: PanelId; by: 1 | -1 }
  | { type: "close"; session: string; paneTab: string }
  /** `to` is the index the tab ends at among its panel's tabs; the other panel's tabs keep their places in the session's list. */
  | { type: "move"; session: string; paneTab: string; to: number }
  /**
   * A web address: a new browser tab, or the tab already showing it (about:blank always makes a new one); shows the pane with the
   * Browser panel in it. With `background` it never changes `open` or the stack, and selects the tab only when Browser had none.
   */
  | { type: "openPage"; session: string; url: string; background?: true }
  /**
   * `marshell open`: a tab added behind the one on screen, or the tab already showing it, marked as new. It never changes `open`. It
   * selects the tab only in a session that had none. `onScreen`: the pane is open with this session shown.
   */
  | { type: "openBackground"; session: string; kind: "file" | "browser"; target: string; by: string; project: string; onScreen: boolean }
  /** The panel's selected tab is in front of the person: it loses its "new" mark (`opened_by` stays). */
  | { type: "seen"; session: string; panel: PanelId }
  /** A browser tab's page moved or got a title: the tab's entry follows it. */
  | { type: "page"; session: string; paneTab: string; url: string; title: string | null }
  /** Shows the pane; writes the stack of `session` down when none is saved. */
  | { type: "show"; session: string | null }
  /** Hides the pane; the stack stays. */
  | { type: "hide" }
  | { type: "toggle"; session: string | null }
  /** A panel's own toggle: its key, its button or its x. */
  | { type: "panel"; panel: PanelId; session: string | null }
  /** The panel's fill button: it fills the pane, or stops filling it. */
  | { type: "fill"; panel: PanelId; session: string | null }
  /** The divider moved: whole weights, 1 to 100000, merged into the stack's. */
  | { type: "weights"; weights: Weights }
  /** Writes the stack down when none is saved. */
  | { type: "pin"; session: string }
  | { type: "width"; width: number | null };

const NO_TABS: SessionTabs = { tabs: [], active: null };

/** The session's tabs; an empty set when it has none. */
export function tabsOf(s: PaneView, session: string | null | undefined): SessionTabs {
  return (session != null && s.sessions[session]) || NO_TABS;
}

/** The panel of the session's `active` tab, else of its first tab, else Files: where the one "selected tab" of a caller that has no panel in hand is. */
export function selectedPanel(s: PaneView, session: string | null | undefined): PanelId {
  const t = tabsOf(s, session);
  const tab = t.tabs.find((x) => x.id === t.active) ?? t.tabs[0];
  return tab ? panelOf(tab) : "files";
}

/** The panel's selected tab in the session. */
export function activeTab(s: PaneView, session: string | null | undefined, panel: PanelId): PaneTab | undefined {
  const t = tabsOf(s, session);
  const id = activeIn(t, panel);
  return id === null ? undefined : t.tabs.find((x) => x.id === id);
}

/** Where the tab is now: the entry its history index points at. */
export function currentEntry(t: PaneTab): PaneEntry {
  return t.history[t.index]!;
}

const isWindowsPath = (p: string) => /^[a-zA-Z]:/.test(p) || p.startsWith("\\\\");

/** Two paths name one file: a Windows path ignores case and the kind of slash; any other path compares exactly. */
export function samePath(a: string, b: string): boolean {
  if (a === b) return true;
  if (!isWindowsPath(a) || !isWindowsPath(b)) return false;
  const norm = (p: string) => p.replace(/\\/g, "/").toLowerCase();
  return norm(a) === norm(b);
}

/** A browser tab: its one history entry holds the page's address. */
export const isBrowserTab = (t: PaneTab): boolean => t.kind === "browser";

/** A file tab showing this path; a browser tab never matches a path. */
const tabHas = (t: PaneTab, path: string) => !isBrowserTab(t) && samePath(currentEntry(t).path, path);

/** A browser tab showing this address; about:blank is never "already open". */
const tabShows = (t: PaneTab, url: string) => isBrowserTab(t) && url !== BLANK && sameUrl(currentEntry(t).path, url);

/** True when `open` or `openPage` would be refused: the session has 32 tabs and none shows this path or address. */
export function openRefused(s: PaneView, session: string, path: string): boolean {
  const { tabs } = tabsOf(s, session);
  if (tabs.length < PANE_LIMITS.tabs) return false;
  return isPageUrl(path) ? !tabs.some((t) => tabShows(t, path)) : !tabs.some((t) => tabHas(t, path));
}

export const lastSegment = (p: string) => p.split(/[/\\]/).filter((x) => x !== "").pop() ?? p;
const folderOf = (p: string) => p.split(/[/\\]/).filter((x) => x !== "").slice(-2, -1)[0] ?? "";

/** Each tab's title: a browser tab's page title, else the file's name, plus " — folder" for file tabs that share a name. */
export function tabTitles(tabs: readonly PaneTab[]): Map<string, string> {
  const names = tabs.map((t) => (isBrowserTab(t) ? null : lastSegment(currentEntry(t).path)));
  const out = new Map<string, string>();
  tabs.forEach((t, i) => {
    if (isBrowserTab(t)) {
      out.set(t.id, pageTitle(t.title, currentEntry(t).path));
      return;
    }
    const shared = names.filter((n) => n === names[i]).length > 1;
    const folder = folderOf(currentEntry(t).path);
    out.set(t.id, shared && folder ? `${names[i]} — ${folder}` : names[i]!);
  });
  return out;
}

/** The project's recent paths, newest first, without those open in the session and without any web address: at most 10. */
export function recentFor(s: PaneView, project: string, session: string): string[] {
  const openPaths = tabsOf(s, session).tabs.filter((t) => !isBrowserTab(t)).map((t) => currentEntry(t).path);
  return s.recent
    .filter((r) => r.project === project && !isPageUrl(r.path) && !openPaths.some((p) => samePath(p, r.path)))
    .slice(0, PANE_LIMITS.shownRecent)
    .map((r) => r.path);
}

/** The tab ids to keep loaded: `active` first, then the previous ones still open, at most 4. */
export function keptTabs(prev: readonly string[], active: string | null, open: readonly string[]): string[] {
  if (active === null) return [];
  const live = new Set(open);
  return [active, ...prev.filter((id) => id !== active && live.has(id))].slice(0, PANE_LIMITS.kept);
}

/**
 * The kept tabs that are drawn, in the order of the tabs: one for each file. Two tabs can come to show one file (a link followed
 * in one, Back, Forward, a layout saved that way), and a file has one document, one draft and one dot: a second editor on it would
 * write its draft over the first one's and its save would take the dot off both. The selected tab is the one drawn for its file;
 * for any other file it is the first kept tab that shows it. The one left out is drawn when it is selected, and opens on the draft
 * the other left.
 */
export function drawnTabs(tabs: readonly PaneTab[], kept: readonly string[], active: string | null): string[] {
  const held = new Set(kept);
  const fileOf = (t: PaneTab) => (isBrowserTab(t) ? null : currentEntry(t).path);
  const selected = tabs.find((t) => t.id === active);
  const taken: string[] = selected && fileOf(selected) !== null ? [fileOf(selected)!] : [];
  const drawn: string[] = [];
  for (const t of tabs) {
    if (!held.has(t.id)) continue;
    const file = fileOf(t);
    if (file !== null && t.id !== active) {
      if (taken.some((p) => samePath(p, file))) continue;
      taken.push(file);
    }
    drawn.push(t.id);
  }
  return drawn;
}

/** The recent list with this file first; the same array when it is already first. */
function recentAfter(list: readonly RecentFile[], project: string, path: string): readonly RecentFile[] {
  const first = list[0];
  if (first && first.project === project && first.path === path) return list;
  const rest = list.filter((r) => !(r.project === project && samePath(r.path, path)));
  return [{ project, path }, ...rest].slice(0, PANE_LIMITS.recent);
}

function withRecent(s: PaneView, project: string, path: string): PaneView {
  const recent = recentAfter(s.recent, project, path) as RecentFile[];
  return recent === s.recent ? s : { ...s, recent };
}

function nextId(tabs: readonly PaneTab[]): string {
  let top = 0;
  for (const t of tabs) {
    const m = /^t(\d+)$/.exec(t.id);
    if (m) top = Math.max(top, Number(m[1]));
  }
  return `t${top + 1}`;
}

/**
 * Rewrites both selection fields from `activeIn`, so the layout holds what the core accepts: while a session has tabs `active` names
 * one of them (Files' selected tab, else Browser's), and `active_browser` is there only while Browser has a tab, naming its selected
 * one. It runs on a session before a change (an old layout's implied selections are written down before one of them changes) and
 * after every write of its tabs or selection. The same object when nothing changes.
 */
function settle(st: SessionTabs): SessionTabs {
  const files = activeIn(st, "files");
  const browser = activeIn(st, "browser");
  const active = files ?? browser;
  if (st.active === active && "active_browser" in st === (browser !== null) && (browser === null || st.active_browser === browser)) return st;
  const { active_browser: _old, ...rest } = st;
  return browser === null ? { ...rest, active } : { ...rest, active, active_browser: browser };
}

/** The tabs with the panel's selection set to this tab (not yet settled). */
function pick(st: SessionTabs, panel: PanelId, id: string): SessionTabs {
  return panel === "browser" ? { ...st, active_browser: id } : { ...st, active: id };
}

function setTabs(s: PaneView, session: string, next: SessionTabs): PaneView {
  return { ...s, sessions: { ...s.sessions, [session]: settle(next) } };
}

/** A stack as the layout holds it. */
const panelsOf = (stack: Stack): Panels => ({ shown: [...stack.shown], weights: { ...stack.weights }, filled: stack.filled });

/**
 * The pane is open with this panel on screen. A hidden pane shows the panel alone, keeping the weights. An open one adds the panel
 * to what it shows (in the stack's order), ends another panel's fill and keeps this panel's own. The stack is written down.
 */
function reveal(s: PaneView, session: string, panel: PanelId): PaneView {
  const stack = stackOf(s, session);
  if (!s.open) return { ...s, open: true, panels: panelsOf({ shown: [panel], weights: stack.weights, filled: null }) };
  const shown = stack.shown.includes(panel) ? stack.shown : PANELS.filter((p) => p === panel || stack.shown.includes(p));
  return { ...s, panels: panelsOf({ shown, weights: stack.weights, filled: stack.filled === panel ? panel : null }) };
}

/** The tabs with this one no longer marked new (the same object when it isn't). */
function seenTab(cur: SessionTabs, id: string): SessionTabs {
  if (!cur.tabs.some((t) => t.id === id && t.unseen)) return cur;
  return { ...cur, tabs: cur.tabs.map((t) => (t.id === id ? { ...t, unseen: false } : t)) };
}

/**
 * A line or column as the layout can hold it (a u32 in the core), or null. Output such as `app.log:1731234567890` reads as a path
 * and a line; a number the core can't hold would make it refuse every save of the layout while the tab is open.
 */
function place(n: number | null | undefined): number | null {
  return typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffff_ffff ? n : null;
}

const TOP_MAX = 10_000;

/** A place as an entry holds it: the page at least 1 and a whole number, the top in ten-thousandths, clamped. */
function placeFields(p: Place): { page: number; top: number } {
  const page = Number.isFinite(p.page) ? Math.min(Math.max(Math.trunc(p.page), 1), 0xffff_ffff) : 1;
  const top = Number.isFinite(p.top) ? Math.min(Math.max(Math.round(p.top * TOP_MAX), 0), TOP_MAX) : 0;
  return { page, top };
}

/** The place an entry keeps, or null when it has no page. */
export function placeOf(e: PaneEntry): Place | null {
  return e.page == null ? null : { page: e.page, top: (e.top ?? 0) / TOP_MAX };
}

function entryOf(target: OpenTarget): PaneEntry {
  const entry: PaneEntry = { path: target.path, line: place(target.line), col: place(target.col), view: target.view ?? "rendered" };
  return target.place ? { ...entry, ...placeFields(target.place) } : entry;
}

/** Replaces one tab; the same state when `edit` returns the tab it was given. */
function editTab(s: PaneView, session: string, paneTab: string, edit: (t: PaneTab) => PaneTab): PaneView {
  const cur = s.sessions[session];
  const old = cur?.tabs.find((t) => t.id === paneTab);
  if (!cur || !old) return s;
  const next = edit(old);
  if (next === old) return s;
  return setTabs(s, session, { ...settle(cur), tabs: cur.tabs.map((t) => (t === old ? next : t)) });
}

/** Like `editTab`, for a file tab: a browser tab has no history to move through and no view to switch. */
function editFileTab(s: PaneView, session: string, paneTab: string, edit: (t: PaneTab) => PaneTab): PaneView {
  return editTab(s, session, paneTab, (t) => (isBrowserTab(t) ? t : edit(t)));
}

/** A title as the layout keeps it: at most 200 characters. */
const TITLE_MAX = 200;
const cutTitle = (title: string | null): string | null =>
  title === null || title.length <= TITLE_MAX ? title : Array.from(title).slice(0, TITLE_MAX).join("");

function openPage(s: PaneView, session: string, url: string, background: boolean): PaneView {
  if (!isPageUrl(url)) return s;
  const cur = settle(tabsOf(s, session));
  const existing = cur.tabs.find((t) => tabShows(t, url));
  if (existing) {
    if (background) return s;
    const seen = seenTab(cur, existing.id);
    if (seen === cur && onScreen(s, session).includes("browser") && activeIn(cur, "browser") === existing.id) return s;
    return setTabs(reveal(s, session, "browser"), session, pick(seen, "browser", existing.id));
  }
  if (cur.tabs.length >= PANE_LIMITS.tabs) return s;
  const tab: PaneTab = { id: nextId(cur.tabs), kind: "browser", history: [entryOf({ path: url })], index: 0, title: null, opened_by: null, unseen: false };
  const added = { ...cur, tabs: [...cur.tabs, tab] };
  // A background tab is selected only when Browser had none; `settle` does that.
  return background ? setTabs(s, session, added) : setTabs(reveal(s, session, "browser"), session, pick(added, "browser", tab.id));
}

function pageMoved(s: PaneView, session: string, paneTab: string, url: string, title: string | null): PaneView {
  if (!isPageUrl(url)) return s;
  const cut = cutTitle(title);
  return editTab(s, session, paneTab, (t) => {
    if (!isBrowserTab(t)) return t;
    if (currentEntry(t).path === url && t.index === 0 && t.history.length === 1 && t.title === cut) return t;
    return { ...t, history: [entryOf({ path: url })], index: 0, title: cut };
  });
}

function open(s: PaneView, session: string, project: string, target: OpenTarget): PaneView {
  const cur = settle(tabsOf(s, session));
  const existing = cur.tabs.find((t) => tabHas(t, target.path));
  let next: PaneView;
  if (existing) {
    let tab = existing.unseen ? { ...existing, unseen: false } : existing;
    const line = place(target.line);
    if (line !== null) {
      const e = currentEntry(existing);
      const col = place(target.col);
      if (e.line !== line || e.col !== col) {
        tab = { ...tab, history: existing.history.map((x, i) => (i === existing.index ? { ...x, line, col } : x)) };
      }
    }
    const panel = panelOf(existing);
    if (tab === existing && onScreen(s, session).includes(panel) && activeIn(cur, panel) === existing.id) {
      next = s;
    } else {
      const tabs = tab === existing ? cur.tabs : cur.tabs.map((t) => (t === existing ? tab : t));
      next = setTabs(reveal(s, session, panel), session, pick({ ...cur, tabs }, panel, existing.id));
    }
  } else {
    if (cur.tabs.length >= PANE_LIMITS.tabs) return s;
    const tab: PaneTab = { id: nextId(cur.tabs), kind: "file", history: [entryOf(target)], index: 0, title: null, opened_by: null, unseen: false };
    const panel = panelOf(tab);
    next = setTabs(reveal(s, session, panel), session, pick({ ...cur, tabs: [...cur.tabs, tab] }, panel, tab.id));
  }
  return withRecent(next, project, target.path);
}

function openBackground(s: PaneView, a: Extract<PaneAction, { type: "openBackground" }>): PaneView {
  const { session, kind, target, by } = a;
  const cur = settle(tabsOf(s, session));
  const existing = cur.tabs.find((t) => (kind === "browser" ? tabShows(t, target) : tabHas(t, target)));
  let next: PaneView;
  if (existing) {
    // The tab in front of the person needs no mark.
    const front = a.onScreen && activeIn(cur, panelOf(existing)) === existing.id;
    const marked = front ? existing : existing.opened_by === by && existing.unseen ? existing : { ...existing, opened_by: by, unseen: true };
    next = marked === existing ? s : setTabs(s, session, { ...cur, tabs: cur.tabs.map((t) => (t === existing ? marked : t)) });
  } else {
    if (cur.tabs.length >= PANE_LIMITS.tabs) return s;
    const tab: PaneTab = { id: nextId(cur.tabs), kind, history: [entryOf({ path: target })], index: 0, title: null, opened_by: by, unseen: true };
    // `settle` selects it when its panel had no tab.
    next = setTabs(s, session, { ...cur, tabs: [...cur.tabs, tab] });
  }
  return kind === "file" ? withRecent(next, a.project, target) : next;
}

function navigate(s: PaneView, session: string, paneTab: string, project: string, target: OpenTarget): PaneView {
  const source = s.sessions[session]?.tabs.find((t) => t.id === paneTab);
  if (!source || isBrowserTab(source)) return s;
  // A link to a file of the other panel opens a tab there and leaves this tab's history alone.
  if (panelOfPath(target.path) !== panelOf(source)) return open(s, session, project, target);
  const moved = editFileTab(s, session, paneTab, (t) => {
    const history = [...t.history.slice(0, t.index + 1), entryOf(target)].slice(-PANE_LIMITS.history);
    return { ...t, history, index: history.length - 1 };
  });
  return moved === s ? s : withRecent(moved, project, target.path);
}

/** Back or Forward: the tab moves through its history, and one that lands on an entry of the other panel goes there, selected. */
function goThrough(s: PaneView, session: string, paneTab: string, by: 1 | -1): PaneView {
  const cur = s.sessions[session];
  const old = cur?.tabs.find((t) => t.id === paneTab);
  if (!cur || !old || isBrowserTab(old)) return s;
  const index = old.index + by;
  if (index < 0 || index >= old.history.length) return s;
  const tab = { ...old, index };
  const panel = panelOf(tab);
  const tabs = cur.tabs.map((t) => (t === old ? tab : t));
  if (panel === panelOf(old)) return setTabs(s, session, { ...settle(cur), tabs });
  return setTabs(reveal(s, session, panel), session, pick(seenTab({ ...settle(cur), tabs }, paneTab), panel, paneTab));
}

function close(s: PaneView, session: string, paneTab: string): PaneView {
  const cur = s.sessions[session];
  const at = cur ? cur.tabs.findIndex((t) => t.id === paneTab) : -1;
  if (!cur || at < 0) return s;
  const was = settle(cur);
  const panel = panelOf(cur.tabs[at]!);
  const tabs = cur.tabs.filter((_, i) => i !== at);
  if (tabs.length === 0) {
    const { [session]: _gone, ...sessions } = s.sessions;
    return { ...s, sessions };
  }
  if (activeIn(was, panel) !== paneTab) return setTabs(s, session, { ...was, tabs });
  // The panel's neighbour: the tab that follows it in the panel, else the one before.
  const mine = was.tabs.filter((t) => panelOf(t) === panel);
  const from = mine.findIndex((t) => t.id === paneTab);
  const next = mine[from + 1] ?? mine[from - 1];
  if (!next) return setTabs(s, session, { ...was, tabs });
  return setTabs(s, session, pick(seenTab({ ...was, tabs }, next.id), panel, next.id));
}

/** The stack as `show` and `toggle` leave it: the saved one, or the one the session's tabs imply, written down. */
function shownWith(s: PaneView, session: string | null): PaneView {
  return { ...s, open: true, panels: s.panels ?? panelsOf(stackOf(s, session)) };
}

function panelToggle(s: PaneView, panel: PanelId, session: string | null): PaneView {
  const stack = stackOf(s, session);
  if (!s.open) return { ...s, open: true, panels: panelsOf({ shown: [panel], weights: stack.weights, filled: null }) };
  if (stackPanels(stack).includes(panel)) {
    // The only panel shown: the pane hides and the panel stays shown.
    if (stack.shown.length === 1) return { ...s, open: false, panels: panelsOf({ ...stack, filled: null }) };
    return { ...s, panels: panelsOf({ shown: stack.shown.filter((p) => p !== panel), weights: stack.weights, filled: null }) };
  }
  return { ...s, panels: panelsOf({ shown: PANELS.filter((p) => p === panel || stack.shown.includes(p)), weights: stack.weights, filled: null }) };
}

function fill(s: PaneView, panel: PanelId, session: string | null): PaneView {
  if (!s.open) return s;
  const stack = stackOf(s, session);
  if (stack.shown.length < 2 || !stack.shown.includes(panel)) return s;
  return { ...s, panels: panelsOf({ ...stack, filled: stack.filled === panel ? null : panel }) };
}

/** The 1 to 100000 whole weight of a number, or null for one that is not a finite number. */
function weightOf(n: unknown): number | null {
  return typeof n === "number" && Number.isFinite(n) ? Math.min(100_000, Math.max(1, Math.round(n))) : null;
}

/** Merges the divider's weights into the saved stack. With no stack saved there is nothing to merge into (`pin` writes one first). */
function mergeWeights(s: PaneView, weights: Weights): PaneView {
  if (!s.panels) return s;
  const stack = stackOf(s, null);
  const merged: Weights = { ...stack.weights };
  let changed = false;
  for (const [key, value] of Object.entries(weights)) {
    const w = weightOf(value);
    if (!isPanelId(key) || w === null) continue;
    if (merged[key] !== w) changed = true;
    merged[key] = w;
  }
  return changed ? { ...s, panels: panelsOf({ ...stack, weights: merged }) } : s;
}

export function paneReducer(s: PaneView, a: PaneAction): PaneView {
  switch (a.type) {
    case "open":
      return open(s, a.session, a.project, a.target);
    case "navigate":
      return navigate(s, a.session, a.paneTab, a.project, a.target);
    case "openPage":
      return openPage(s, a.session, a.url, a.background === true);
    case "openBackground":
      return openBackground(s, a);
    case "seen": {
      const cur = settle(tabsOf(s, a.session));
      const id = activeIn(cur, a.panel);
      if (id === null) return s;
      const shown = seenTab(cur, id);
      return shown === cur ? s : setTabs(s, a.session, shown);
    }
    case "page":
      return pageMoved(s, a.session, a.paneTab, a.url, a.title);
    case "back":
      return goThrough(s, a.session, a.paneTab, -1);
    case "forward":
      return goThrough(s, a.session, a.paneTab, 1);
    case "setView":
      return editFileTab(s, a.session, a.paneTab, (t) =>
        currentEntry(t).view === a.view ? t : { ...t, history: t.history.map((e, i) => (i === t.index ? { ...e, view: a.view } : e)) },
      );
    case "place": {
      const fields = placeFields(a.place);
      return editFileTab(s, a.session, a.paneTab, (t) => {
        const e = currentEntry(t);
        return e.page === fields.page && e.top === fields.top
          ? t
          : { ...t, history: t.history.map((x, i) => (i === t.index ? { ...x, ...fields } : x)) };
      });
    }
    case "select": {
      const raw = s.sessions[a.session];
      const tab = raw?.tabs.find((t) => t.id === a.paneTab);
      if (!raw || !tab) return s;
      const cur = settle(raw);
      const panel = panelOf(tab);
      // Choosing the tab already selected is still the person's first look at it.
      const shown = seenTab(cur, a.paneTab);
      if (activeIn(cur, panel) === a.paneTab && shown === raw) return s;
      return setTabs(s, a.session, pick(shown, panel, a.paneTab));
    }
    case "step": {
      const raw = s.sessions[a.session];
      if (!raw) return s;
      const cur = settle(raw);
      const mine = cur.tabs.filter((t) => panelOf(t) === a.panel);
      if (mine.length === 0) return s;
      const here = activeIn(cur, a.panel);
      const at = mine.findIndex((t) => t.id === here);
      const n = mine.length;
      const id = mine[(((at < 0 ? 0 : at + a.by) % n) + n) % n]!.id;
      return id === here ? s : setTabs(s, a.session, pick(seenTab(cur, id), a.panel, id));
    }
    case "close":
      return close(s, a.session, a.paneTab);
    case "move": {
      const raw = s.sessions[a.session];
      const tab = raw?.tabs.find((t) => t.id === a.paneTab);
      if (!raw || !tab || !Number.isFinite(a.to)) return s;
      const panel = panelOf(tab);
      const mine = raw.tabs.filter((t) => panelOf(t) === panel);
      const from = mine.indexOf(tab);
      const to = Math.min(mine.length - 1, Math.max(0, Math.round(a.to)));
      if (to === from) return s;
      const order = mine.filter((t) => t !== tab);
      order.splice(to, 0, tab);
      // The panel's tabs fill the places the panel held; the other panel's tabs stay where they were.
      let n = 0;
      const tabs = raw.tabs.map((t) => (panelOf(t) === panel ? order[n++]! : t));
      return setTabs(s, a.session, { ...settle(raw), tabs });
    }
    case "show":
      return s.open ? s : shownWith(s, a.session);
    case "hide":
      return s.open ? { ...s, open: false } : s;
    case "toggle":
      return s.open ? { ...s, open: false } : shownWith(s, a.session);
    case "panel":
      return panelToggle(s, a.panel, a.session);
    case "fill":
      return fill(s, a.panel, a.session);
    case "weights":
      return mergeWeights(s, a.weights);
    case "pin":
      return s.panels ? s : { ...s, panels: panelsOf(stackOf(s, a.session)) };
    case "width": {
      if (a.width === null) return s.width === null ? s : { ...s, width: null };
      if (!Number.isFinite(a.width)) return s;
      const width = Math.round(Math.min(WIDTH_MAX, Math.max(WIDTH_MIN, a.width)));
      return width === s.width ? s : { ...s, width };
    }
  }
}
