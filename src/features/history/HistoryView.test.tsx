import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Conversation } from "../../generated/Conversation";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import type { SearchHit } from "../../generated/SearchHit";
import { HistoryView, hitKey, type HistoryViewProps } from "./HistoryView";

const NOW = Date.UTC(2026, 9, 23, 12, 0, 0);
const noop = () => {};
const entry = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  agent: "claude",
  session_id: "s1",
  tab_id: "t1",
  cwd: "C:\\dev\\billing",
  title: "Fix the invoice rounding",
  branch: "main",
  model: "opus",
  version: "2.1.300",
  limited: false,
  turns: 3,
  started_ms: NOW - 3_600_000,
  updated_ms: NOW - 300_000,
  ...over,
});
const base: HistoryViewProps = {
  entries: [],
  filters: { when: "any" },
  query: "",
  hits: null,
  selectedId: null,
  conversation: null,
  liveTabFor: () => null,
  now: NOW,
  os: "windows",
  searchFocus: 0,
  onQuery: noop,
  onFilter: noop,
  onSelect: noop,
  onOpen: noop,
  onResume: noop,
  onGoTo: noop,
  onEscape: noop,
};
const html = (over: Partial<HistoryViewProps> = {}) => renderToStaticMarkup(<HistoryView {...base} {...over} />);

const two = [
  entry({ session_id: "a", title: "Fix the invoice rounding", updated_ms: NOW - 60_000 }),
  entry({ session_id: "b", title: "", cwd: "C:\\dev\\airport", branch: undefined, turns: 1, updated_ms: NOW - 7_200_000 }),
  entry({ session_id: "c", title: "Add retries", updated_ms: NOW - 120_000 }),
];

describe("HistoryView list", () => {
  it("has the heading and a labelled search box", () => {
    const out = html();
    expect(out).toMatch(/<h2[^>]*>History<\/h2>/);
    expect(out).toContain("Search conversations");
    expect(out).toContain('placeholder="Search conversations"');
  });

  it("groups the rows by project: one heading each, options in a listbox", () => {
    const out = html({ entries: two });
    expect(out).toContain('role="listbox"');
    expect(out.match(/role="option"/g)).toHaveLength(3);
    expect(out.match(/<h3[^>]*>/g)).toHaveLength(2);
    // Newest group first: billing (a, c) then airport (b).
    expect(out.indexOf(">billing</h3>")).toBeLessThan(out.indexOf(">airport</h3>"));
    // Each group is named by its heading.
    const groups = [...out.matchAll(/role="group" aria-labelledby="([^"]+)"/g)];
    expect(groups).toHaveLength(2);
    for (const m of groups) expect(out).toContain(`id="${m[1]}"`);
  });

  it("shows the row's title or the placeholder, and the meta line", () => {
    const out = html({ entries: two });
    expect(out).toContain("Fix the invoice rounding");
    expect(out).toContain("Untitled conversation");
    expect(out).toContain("billing · main · 1 min ago · 3 turns");
    expect(out).toContain("airport · 2 h ago · 1 turn<");
  });

  it("points aria-activedescendant at the selected option", () => {
    const out = html({ entries: two, selectedId: "c" });
    const listbox = /<[^>]*role="listbox"[^>]*>/.exec(out)?.[0] ?? "";
    const active = /aria-activedescendant="([^"]+)"/.exec(listbox)?.[1];
    expect(active).toBeDefined();
    const selected = [...out.matchAll(/<[^>]*role="option"[^>]*>/g)].map((m) => m[0]).filter((t) => t.includes('aria-selected="true"'));
    expect(selected).toHaveLength(1);
    expect(selected[0]).toContain(`id="${active}"`);
  });

  it("makes the listbox a tab stop, so arrows work from it", () => {
    expect(/<[^>]*role="listbox"[^>]*>/.exec(html({ entries: two }))?.[0]).toContain('tabindex="0"');
  });

  it("says so when there are no conversations, word for word", () => {
    expect(html()).toContain("No conversations yet. Claude Code sessions you run in Marshell show up here when their first turn ends.");
  });

  it("says nothing matches, with the query in curly quotes, escaped", () => {
    expect(html({ query: "rounding", hits: [] })).toContain("Nothing matches “rounding”.");
    expect(html({ query: "<b>x</b>", hits: [] })).toContain("Nothing matches “&lt;b&gt;x&lt;/b&gt;”.");
  });

  it("has the five filters with their labels and the When options", () => {
    const out = html({ entries: two });
    for (const label of ["Agent", "Project", "Branch", "Model", "When"]) expect(out).toMatch(new RegExp(`<label[^>]*><span[^>]*>${label}</span>`));
    for (const w of ["Any time", "Today", "Last 7 days", "Last 30 days"]) expect(out).toContain(`>${w}</option>`);
    expect(out).toContain(">opus</option>");
    expect(out).toContain(">main</option>");
  });

  it("keeps a chosen filter value in its select even when the list no longer has it", () => {
    const out = html({ entries: [], filters: { when: "7d", model: "sonnet" } });
    const options = [...out.matchAll(/<option[^>]*>[^<]*<\/option>/g)].map((m) => m[0]);
    expect(options.find((o) => o.includes('value="sonnet"'))).toContain("selected");
    expect(options.find((o) => o.includes('value="7d"'))).toContain("selected");
  });
});

describe("HistoryView search hits", () => {
  const hit: SearchHit = {
    agent: "claude",
    session_id: "a",
    title: "Fix the invoice rounding",
    cwd: "C:\\dev\\billing",
    turn: 2,
    at_ms: NOW - 1000,
    snippet: "round the \u0002total\u0003 up <img src=x onerror=alert(1)>",
  };

  it("shows a hit's snippet with the match in <mark>, and nothing from it as HTML", () => {
    const out = html({ query: "total", hits: [hit] });
    expect(out).toContain("<mark>total</mark>");
    expect(out).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(out).not.toContain("<img");
    expect(out).not.toContain("\u0002");
    expect(out).not.toContain("\u0003");
  });

  it("lists hits as options in the same listbox, one per session and turn", () => {
    const out = html({ query: "total", hits: [hit, { ...hit, turn: 4 }], selectedId: hitKey({ ...hit, turn: 4 }) });
    expect(out.match(/role="option"/g)).toHaveLength(2);
    expect(out).toContain("billing · turn 3");
    expect(out).not.toContain("No conversations yet");
    expect(out).not.toContain("Nothing matches");
  });
});

describe("HistoryView conversation pane", () => {
  const conv: Conversation = {
    agent: "claude",
    session_id: "a",
    title: "Fix the invoice rounding",
    cwd: "C:\\dev\\billing",
    limited: false,
    turns: [{ index: 0, at_ms: 0, prompt: "hello", items: [] }],
  };

  it("shows the conversation beside the list, and the live tab for a session that is open", () => {
    const out = html({ entries: two, conversation: conv, liveTabFor: (e) => (e.session_id === "a" ? "t1" : null) });
    expect(out).toContain('aria-label="Fix the invoice rounding"');
    expect(out).toContain("Go to session");
    expect(out).not.toContain(">Resume<");
    expect(html({ entries: two, conversation: conv })).toContain(">Resume<");
  });

  it("shows loading, a failure and the no-archive copy as text; the window's one live region (the Announcer) says them", () => {
    const loading = html({ entries: two, conversation: "loading" });
    expect(loading).toContain("Loading conversation…");
    const out = html({ entries: two, conversation: { error: "Marshell has no copy of this conversation yet." } });
    expect(out).toContain("Marshell has no copy of this conversation yet.");
    for (const o of [loading, out, html({ query: "zzz", hits: [] }), html()]) expect(o).not.toMatch(/role="(?:status|alert)"|aria-live/);
  });

  it("marks the layout so a narrow window shows the conversation instead of the list", () => {
    expect(html({ entries: two, conversation: conv })).toContain('data-detail="open"');
    expect(html({ entries: two })).not.toContain('data-detail="open"');
  });
});
