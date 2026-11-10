import type { PaneEntry } from "../../generated/PaneEntry";
import type { PaneTab } from "../../generated/PaneTab";
import type { PaneView } from "../../generated/PaneView";
import type { RecentFile } from "../../generated/RecentFile";
import type { SessionTabs } from "../../generated/SessionTabs";

/** The pane's limits: tabs per session, history entries per tab, recent files saved and shown, and tabs kept loaded. */
export const PANE_LIMITS = { tabs: 32, history: 50, recent: 200, shownRecent: 10, kept: 4 } as const;

/** The narrowest and widest saved width of the pane, in pixels. */
const WIDTH_MIN = 320;
const WIDTH_MAX = 4096;

export type View = "rendered" | "source";
export type OpenTarget = { path: string; line?: number; col?: number; view?: View };
export type PaneAction =
  /** A new tab, or its existing tab; shows the pane. */
  | { type: "open"; session: string; project: string; target: OpenTarget }
  /** A link inside a tab: pushes its history. */
  | { type: "navigate"; session: string; paneTab: string; project: string; target: OpenTarget }
  | { type: "back" | "forward"; session: string; paneTab: string }
  | { type: "setView"; session: string; paneTab: string; view: View }
  | { type: "select"; session: string; paneTab: string }
  /** The next or previous tab, wrapping. */
  | { type: "step"; session: string; by: 1 | -1 }
  | { type: "close"; session: string; paneTab: string }
  /** `to` is the index the tab ends at. */
  | { type: "move"; session: string; paneTab: string; to: number }
  | { type: "show" }
  | { type: "hide" }
  | { type: "toggle" }
  | { type: "width"; width: number | null };

const NO_TABS: SessionTabs = { tabs: [], active: null };

/** The session's tabs; an empty set when it has none. */
export function tabsOf(s: PaneView, session: string | null | undefined): SessionTabs {
  return (session != null && s.sessions[session]) || NO_TABS;
}

/** The session's selected tab. */
export function activeTab(s: PaneView, session: string | null | undefined): PaneTab | undefined {
  const t = tabsOf(s, session);
  return t.tabs.find((x) => x.id === t.active);
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

const tabHas = (t: PaneTab, path: string) => samePath(currentEntry(t).path, path);

/** True when `open` would be refused: the session has 32 tabs and none shows this path. */
export function openRefused(s: PaneView, session: string, path: string): boolean {
  const { tabs } = tabsOf(s, session);
  return tabs.length >= PANE_LIMITS.tabs && !tabs.some((t) => tabHas(t, path));
}

const lastSegment = (p: string) => p.split(/[/\\]/).filter((x) => x !== "").pop() ?? p;
const folderOf = (p: string) => p.split(/[/\\]/).filter((x) => x !== "").slice(-2, -1)[0] ?? "";

/** Each tab's title: the file's name, plus " — folder" for tabs that share a name. */
export function tabTitles(tabs: readonly PaneTab[]): Map<string, string> {
  const names = tabs.map((t) => lastSegment(currentEntry(t).path));
  const out = new Map<string, string>();
  tabs.forEach((t, i) => {
    const shared = names.filter((n) => n === names[i]).length > 1;
    const folder = folderOf(currentEntry(t).path);
    out.set(t.id, shared && folder ? `${names[i]} — ${folder}` : names[i]!);
  });
  return out;
}

/** The project's recent paths, newest first, without those open in the session: at most 10. */
export function recentFor(s: PaneView, project: string, session: string): string[] {
  const openPaths = tabsOf(s, session).tabs.map((t) => currentEntry(t).path);
  return s.recent
    .filter((r) => r.project === project && !openPaths.some((p) => samePath(p, r.path)))
    .slice(0, PANE_LIMITS.shownRecent)
    .map((r) => r.path);
}

/** The tab ids to keep loaded: `active` first, then the previous ones still open, at most 4. */
export function keptTabs(prev: readonly string[], active: string | null, open: readonly string[]): string[] {
  if (active === null) return [];
  const live = new Set(open);
  return [active, ...prev.filter((id) => id !== active && live.has(id))].slice(0, PANE_LIMITS.kept);
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

function setTabs(s: PaneView, session: string, next: SessionTabs): PaneView {
  return { ...s, sessions: { ...s.sessions, [session]: next } };
}

function entryOf(target: OpenTarget): PaneEntry {
  return { path: target.path, line: target.line ?? null, col: target.col ?? null, view: target.view ?? "rendered" };
}

/** Replaces one tab; the same state when `edit` returns the tab it was given. */
function editTab(s: PaneView, session: string, paneTab: string, edit: (t: PaneTab) => PaneTab): PaneView {
  const cur = s.sessions[session];
  const old = cur?.tabs.find((t) => t.id === paneTab);
  if (!cur || !old) return s;
  const next = edit(old);
  if (next === old) return s;
  return setTabs(s, session, { ...cur, tabs: cur.tabs.map((t) => (t === old ? next : t)) });
}

function open(s: PaneView, session: string, project: string, target: OpenTarget): PaneView {
  const cur = tabsOf(s, session);
  const existing = cur.tabs.find((t) => tabHas(t, target.path));
  let next: PaneView;
  if (existing) {
    let tab = existing;
    if (target.line !== undefined) {
      const e = currentEntry(existing);
      const col = target.col ?? null;
      if (e.line !== target.line || e.col !== col) {
        tab = { ...existing, history: existing.history.map((x, i) => (i === existing.index ? { ...x, line: target.line!, col } : x)) };
      }
    }
    if (tab === existing && cur.active === existing.id && s.open) {
      next = s;
    } else {
      const tabs = tab === existing ? cur.tabs : cur.tabs.map((t) => (t === existing ? tab : t));
      next = { ...setTabs(s, session, { tabs, active: existing.id }), open: true };
    }
  } else {
    if (cur.tabs.length >= PANE_LIMITS.tabs) return s;
    const tab: PaneTab = { id: nextId(cur.tabs), kind: "file", history: [entryOf(target)], index: 0 };
    next = { ...setTabs(s, session, { tabs: [...cur.tabs, tab], active: tab.id }), open: true };
  }
  return withRecent(next, project, target.path);
}

function navigate(s: PaneView, session: string, paneTab: string, project: string, target: OpenTarget): PaneView {
  const moved = editTab(s, session, paneTab, (t) => {
    const history = [...t.history.slice(0, t.index + 1), entryOf(target)].slice(-PANE_LIMITS.history);
    return { ...t, history, index: history.length - 1 };
  });
  return moved === s ? s : withRecent(moved, project, target.path);
}

function close(s: PaneView, session: string, paneTab: string): PaneView {
  const cur = s.sessions[session];
  const at = cur ? cur.tabs.findIndex((t) => t.id === paneTab) : -1;
  if (!cur || at < 0) return s;
  const tabs = cur.tabs.filter((_, i) => i !== at);
  if (tabs.length === 0) {
    const { [session]: _gone, ...sessions } = s.sessions;
    return { ...s, sessions };
  }
  const active = cur.active === paneTab ? (tabs[at] ?? tabs[at - 1])!.id : cur.active;
  return setTabs(s, session, { tabs, active });
}

export function paneReducer(s: PaneView, a: PaneAction): PaneView {
  switch (a.type) {
    case "open":
      return open(s, a.session, a.project, a.target);
    case "navigate":
      return navigate(s, a.session, a.paneTab, a.project, a.target);
    case "back":
      return editTab(s, a.session, a.paneTab, (t) => (t.index > 0 ? { ...t, index: t.index - 1 } : t));
    case "forward":
      return editTab(s, a.session, a.paneTab, (t) => (t.index < t.history.length - 1 ? { ...t, index: t.index + 1 } : t));
    case "setView":
      return editTab(s, a.session, a.paneTab, (t) =>
        currentEntry(t).view === a.view ? t : { ...t, history: t.history.map((e, i) => (i === t.index ? { ...e, view: a.view } : e)) },
      );
    case "select": {
      const cur = s.sessions[a.session];
      if (!cur || cur.active === a.paneTab || !cur.tabs.some((t) => t.id === a.paneTab)) return s;
      return setTabs(s, a.session, { ...cur, active: a.paneTab });
    }
    case "step": {
      const cur = s.sessions[a.session];
      if (!cur || cur.tabs.length === 0) return s;
      const at = cur.tabs.findIndex((t) => t.id === cur.active);
      const n = cur.tabs.length;
      const id = cur.tabs[(((at < 0 ? 0 : at + a.by) % n) + n) % n]!.id;
      return id === cur.active ? s : setTabs(s, a.session, { ...cur, active: id });
    }
    case "close":
      return close(s, a.session, a.paneTab);
    case "move": {
      const cur = s.sessions[a.session];
      const from = cur ? cur.tabs.findIndex((t) => t.id === a.paneTab) : -1;
      if (!cur || from < 0 || !Number.isFinite(a.to)) return s;
      const to = Math.min(cur.tabs.length - 1, Math.max(0, Math.round(a.to)));
      if (to === from) return s;
      const tabs = cur.tabs.filter((_, i) => i !== from);
      tabs.splice(to, 0, cur.tabs[from]!);
      return setTabs(s, a.session, { ...cur, tabs });
    }
    case "show":
      return s.open ? s : { ...s, open: true };
    case "hide":
      return s.open ? { ...s, open: false } : s;
    case "toggle":
      return { ...s, open: !s.open };
    case "width": {
      if (a.width === null) return s.width === null ? s : { ...s, width: null };
      if (!Number.isFinite(a.width)) return s;
      const width = Math.round(Math.min(WIDTH_MAX, Math.max(WIDTH_MIN, a.width)));
      return width === s.width ? s : { ...s, width };
    }
  }
}
