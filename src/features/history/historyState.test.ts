import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import { escapeAction, filterOptions, groupByProject, moveSelection, rowMeta, sinceFor } from "./historyState";

const entry = (over: Partial<HistoryEntry>): HistoryEntry => ({
  agent: "claude",
  session_id: "s",
  tab_id: "t",
  cwd: "C:\\dev\\demo",
  title: "T",
  limited: false,
  tags: [],
  pinned: false,
  archived: false,
  bookmarks: 0,
  named: false,
  turns: 2,
  started_ms: 0,
  updated_ms: 0,
  ...over,
});

describe("groupByProject", () => {
  it("groups by cwd, newest group first, labelled by the last path part", () => {
    const groups = groupByProject([
      entry({ session_id: "a", cwd: "C:\\dev\\old", updated_ms: 1 }),
      entry({ session_id: "b", cwd: "/home/me/new/", updated_ms: 9 }),
      entry({ session_id: "c", cwd: "C:\\dev\\old", updated_ms: 5 }),
    ]);
    expect(groups.map((g) => [g.project, g.label, g.entries.map((e) => e.session_id)])).toEqual([
      ["/home/me/new/", "new", ["b"]],
      ["C:\\dev\\old", "old", ["c", "a"]],
    ]);
  });
});

describe("filterOptions", () => {
  it("sorts and dedupes each list, skipping unset values", () => {
    expect(
      filterOptions([
        entry({ agent: "codex", cwd: "b", branch: "main", model: "x" }),
        entry({ agent: "claude", cwd: "a", branch: "dev" }),
        entry({ agent: "claude", cwd: "a", branch: "main", model: "x" }),
      ]),
    ).toEqual({ agents: ["claude", "codex"], projects: ["a", "b"], branches: ["dev", "main"], models: ["x"] });
  });
});

describe("sinceFor", () => {
  const now = new Date(2026, 9, 23, 15, 30).getTime();
  it("maps each choice", () => {
    expect(sinceFor("any", now)).toBeUndefined();
    expect(sinceFor("today", now)).toBe(new Date(2026, 9, 23).getTime());
    expect(sinceFor("7d", now)).toBe(now - 7 * 86_400_000);
    expect(sinceFor("30d", now)).toBe(now - 30 * 86_400_000);
  });
});

describe("rowMeta", () => {
  const now = 10_000_000_000;
  it("joins project, branch, age and turns", () => {
    expect(rowMeta(entry({ branch: "main", turns: 1, updated_ms: now - 2 * 3_600_000 }), now)).toBe(
      "demo · main · 2 h ago · 1 turn",
    );
  });
  it("leaves out a missing branch and pluralises turns", () => {
    expect(rowMeta(entry({ turns: 3, updated_ms: now - 5 * 60_000 }), now)).toBe("demo · 5 min ago · 3 turns");
    expect(rowMeta(entry({ turns: 0, updated_ms: now - 10_000 }), now)).toBe("demo · just now · 0 turns");
    expect(rowMeta(entry({ turns: 2, updated_ms: now - 3 * 86_400_000 }), now)).toBe("demo · 3 d ago · 2 turns");
  });
});

describe("moveSelection", () => {
  const ids = ["a", "b", "c"];
  it("starts from the ends when nothing is selected", () => {
    expect(moveSelection(ids, null, "ArrowDown")).toBe("a");
    expect(moveSelection(ids, null, "Home")).toBe("a");
    expect(moveSelection(ids, null, "ArrowUp")).toBe("c");
    expect(moveSelection(ids, null, "End")).toBe("c");
  });
  it("moves and stops at the ends", () => {
    expect(moveSelection(ids, "a", "ArrowDown")).toBe("b");
    expect(moveSelection(ids, "c", "ArrowDown")).toBe("c");
    expect(moveSelection(ids, "a", "ArrowUp")).toBe("a");
    expect(moveSelection(ids, "b", "End")).toBe("c");
    expect(moveSelection(ids, "b", "Home")).toBe("a");
  });
  it("gives null for no ids", () => {
    expect(moveSelection([], null, "ArrowDown")).toBeNull();
    expect(moveSelection([], "a", "End")).toBeNull();
  });
});

describe("escapeAction", () => {
  it("closes the conversation, then clears the search, then closes History", () => {
    expect(escapeAction({ conversationOpen: true, query: "x" })).toBe("close-conversation");
    expect(escapeAction({ conversationOpen: false, query: "x" })).toBe("clear-search");
    expect(escapeAction({ conversationOpen: false, query: "" })).toBe("close-history");
  });
});
