import type { Client } from "../../generated/Client";
import type { ImportStatus } from "../../generated/ImportStatus";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { HistoryQuery } from "../../generated/HistoryQuery";
import type { HistoryFilters } from "./HistoryView";

const DAY_MS = 86_400_000;

/** The last part of a folder path, split on both separators, ignoring a trailing one. */
const folderName = (cwd: string) => cwd.split(/[\\/]/).filter(Boolean).pop() ?? cwd;

/** The key of the group of pinned conversations: not a folder path, so no project can collide with it. */
export const PINNED_GROUP = "\u0000pinned";

/** Pinned conversations first, in one group, then the projects, newest group first. A pinned entry appears only in the Pinned group. */
export function groupByProject(entries: HistoryEntry[]): { project: string; label: string; entries: HistoryEntry[] }[] {
  const groups = new Map<string, HistoryEntry[]>();
  const pinned: HistoryEntry[] = [];
  for (const e of [...entries].sort((a, b) => b.updated_ms - a.updated_ms)) {
    if (e.pinned) {
      pinned.push(e);
      continue;
    }
    const g = groups.get(e.cwd);
    if (g) g.push(e);
    else groups.set(e.cwd, [e]);
  }
  // Entries are visited newest first, so insertion order already puts the newest group first.
  const projects = [...groups].map(([project, es]) => ({ project, label: folderName(project), entries: es }));
  return pinned.length > 0 ? [{ project: PINNED_GROUP, label: "Pinned", entries: pinned }, ...projects] : projects;
}

const sortedUnique = (values: (string | undefined)[]) =>
  [...new Set(values.filter((v): v is string => !!v))].sort((a, b) => a.localeCompare(b));

export function filterOptions(entries: HistoryEntry[]) {
  return {
    agents: sortedUnique(entries.map((e) => e.agent)),
    projects: sortedUnique(entries.map((e) => e.cwd)),
    branches: sortedUnique(entries.map((e) => e.branch)),
    models: sortedUnique(entries.map((e) => e.model)),
    tags: sortedUnique(entries.flatMap((e) => e.tags)),
  };
}

export function sinceFor(when: "any" | "today" | "7d" | "30d", now: number): number | undefined {
  if (when === "any") return undefined;
  if (when === "today") {
    const d = new Date(now);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  }
  return now - (when === "7d" ? 7 : 30) * DAY_MS;
}

/** How long ago, from an elapsed time in ms: "just now", "5 min ago", "2 h ago", "3 d ago". */
export function relativeTime(ms: number): string {
  const min = Math.floor(Math.max(0, ms) / 60_000);
  if (min < 1) return "just now";
  if (min < 60) return `${min} min ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} h ago`;
  return `${Math.floor(h / 24)} d ago`;
}

export function rowMeta(e: HistoryEntry, now: number): string {
  return [
    folderName(e.cwd),
    e.branch,
    relativeTime(now - e.updated_ms),
    `${e.turns} ${e.turns === 1 ? "turn" : "turns"}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

/** What the list asks the core for. `bookmarked` is left out: the Bookmarked filter shows the bookmarks route's rows instead of the list. */
export function listQuery(filters: HistoryFilters, now: number): HistoryQuery {
  const { when, bookmarked: _bookmarked, place, headless, ...rest } = filters;
  return { ...rest, ...(place !== "any" && { place }), ...(headless && { headless }), since_ms: sinceFor(when, now) };
}

const RECENT_MS = 10 * 60_000;

/** The question before resuming a conversation that ran outside Marshell and changed under 10 minutes ago, so it may still be open in another terminal. Null when none applies. */
export function resumeWarning(client: Client, updatedMs: number, now: number): string | null {
  const age = now - updatedMs;
  if (client === "marshell" || age >= RECENT_MS) return null;
  const minutes = Math.floor(age / 60_000);
  const when = minutes < 1 ? "less than a minute ago" : `${minutes} min ago`;
  return `This conversation changed ${when} and may still be open in another terminal. Resume here anyway?`;
}

/** The mark on a row for where the conversation ran; none for one run in Marshell. */
export function clientMark(c: Client): string | null {
  switch (c) {
    case "marshell":
      return null;
    case "terminal":
      return "Outside Marshell";
    case "vscode":
      return "VS Code";
    case "desktop":
      return "Desktop app";
    case "headless":
      return "Headless";
  }
}

const AGENT_NAMES: Record<string, string> = { claude: "Claude Code", codex: "Codex CLI" };
/** An agent's name in the Agent filter; an agent not named here shows its id. */
export const historyAgentLabel = (agent: string): string => AGENT_NAMES[agent] ?? agent;

/** The progress line while the import copies files; null when it isn't copying (or not yet known). */
export const importLine = (s: ImportStatus | null): string | null =>
  s?.copying ? `Copying conversations from Claude Code and Codex: ${s.done} of ${s.total}.` : null;

/** What's spoken when a copy ends: finished, or stopped by the switch with how far it got; null when the status couldn't be read. */
export const importEndLine = (s: ImportStatus | null): string | null => {
  if (!s) return null;
  return s.on ? "Finished copying conversations from Claude Code and Codex." : `Stopped copying conversations from Claude Code and Codex: ${s.done} of ${s.total} copied.`;
};

/** What the Bookmarked filter asks the core for: the list's filters, one row per bookmark. */
export const bookmarkQuery = (filters: HistoryFilters, now: number): HistoryQuery => ({ ...listQuery(filters, now), bookmarked: true });

export function moveSelection(
  ids: string[],
  current: string | null,
  key: "ArrowUp" | "ArrowDown" | "Home" | "End",
): string | null {
  if (ids.length === 0) return null;
  const last = ids.length - 1;
  if (key === "Home") return ids[0]!;
  if (key === "End") return ids[last]!;
  const at = current === null ? -1 : ids.indexOf(current);
  if (at < 0) return key === "ArrowDown" ? ids[0]! : ids[last]!;
  return ids[Math.min(last, Math.max(0, at + (key === "ArrowDown" ? 1 : -1)))]!;
}

export function escapeAction(state: {
  conversationOpen: boolean;
  query: string;
}): "close-conversation" | "clear-search" | "close-history" {
  if (state.conversationOpen) return "close-conversation";
  return state.query !== "" ? "clear-search" : "close-history";
}
