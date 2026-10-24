import { describe, expect, it } from "vitest";
import type { HistoryEntry } from "../../generated/HistoryEntry";
import { bookmarkQuery, clientMark, historyAgentLabel, importLine, escapeAction, filterOptions, groupByProject, listQuery, moveSelection, resumeWarning, rowMeta, sinceFor } from "./historyState";

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
  client: "marshell",
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
    ).toEqual({ agents: ["claude", "codex"], projects: ["a", "b"], branches: ["dev", "main"], models: ["x"], tags: [] });
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

describe("groupByProject with pins", () => {
  it("puts pinned first and only once", () => {
    const groups = groupByProject([
      entry({ session_id: "a", cwd: "C:\\dev\\old", updated_ms: 1, pinned: true }),
      entry({ session_id: "b", cwd: "C:\\dev\\new", updated_ms: 9 }),
      entry({ session_id: "c", cwd: "C:\\dev\\old", updated_ms: 5 }),
      entry({ session_id: "d", cwd: "C:\\dev\\new", updated_ms: 7, pinned: true }),
    ]);
    expect(groups.map((g) => [g.project, g.label, g.entries.map((e) => e.session_id)])).toEqual([
      ["\u0000pinned", "Pinned", ["d", "a"]],
      ["C:\\dev\\new", "new", ["b"]],
      ["C:\\dev\\old", "old", ["c"]],
    ]);
    expect(groups.flatMap((g) => g.entries)).toHaveLength(4);
  });

  it("drops a project group whose entries are all pinned, and has no Pinned group when nothing is pinned", () => {
    expect(groupByProject([entry({ session_id: "a", pinned: true })]).map((g) => g.label)).toEqual(["Pinned"]);
    expect(groupByProject([entry({ session_id: "a" })]).map((g) => g.label)).toEqual(["demo"]);
  });
});

describe("filterOptions tags", () => {
  it("offers each tag once, sorted", () => {
    expect(
      filterOptions([entry({ tags: ["zeta", "bug"] }), entry({ tags: ["bug", "alpha"] }), entry({ tags: [] })]).tags,
    ).toEqual(["alpha", "bug", "zeta"]);
  });
});

describe("the queries the view sends", () => {
  const filters = { when: "any", tag: "bug", bookmarked: true, archived: true, agent: "claude", place: "any", headless: false } as const;
  it("passes archived=true and the tag to the list, which never filters by bookmarks itself", () => {
    expect(listQuery(filters, 0)).toEqual({ agent: "claude", tag: "bug", archived: true, since_ms: undefined });
    expect(listQuery({ when: "any", bookmarked: false, archived: false, place: "any", headless: false }, 0).archived).toBe(false);
  });
  it("asks the bookmarks route with the same filters and bookmarked=true", () => {
    expect(bookmarkQuery(filters, 0)).toEqual({ agent: "claude", tag: "bug", archived: true, bookmarked: true, since_ms: undefined });
  });
});

describe("where a conversation ran", () => {
  const f = { when: "any", bookmarked: false, archived: false, place: "any", headless: false } as const;
  it("sends place only when it is not Any, and headless only when on", () => {
    expect(listQuery({ ...f, place: "outside" }, 0).place).toBe("outside");
    expect(listQuery({ ...f, place: "marshell" }, 0).place).toBe("marshell");
    expect("place" in listQuery(f, 0)).toBe(false);
    expect(listQuery({ ...f, headless: true }, 0).headless).toBe(true);
    expect("headless" in listQuery(f, 0)).toBe(false);
  });
  it("marks each client but Marshell", () => {
    expect(clientMark("marshell")).toBeNull();
    expect(clientMark("terminal")).toBe("Outside Marshell");
    expect(clientMark("vscode")).toBe("VS Code");
    expect(clientMark("desktop")).toBe("Desktop app");
    expect(clientMark("headless")).toBe("Headless");
  });
  it("names the agents", () => {
    expect(historyAgentLabel("claude")).toBe("Claude Code");
    expect(historyAgentLabel("codex")).toBe("Codex CLI");
    expect(historyAgentLabel("other")).toBe("other");
  });
  it("words the import's progress, or says nothing when it isn't copying", () => {
    expect(importLine({ copying: true, done: 120, total: 355, on: true })).toBe("Copying conversations from Claude Code and Codex: 120 of 355.");
    expect(importLine({ copying: false, done: 355, total: 355, on: true })).toBeNull();
    expect(importLine(null)).toBeNull();
  });
});

describe("resumeWarning", () => {
  const now = 10_000_000;
  const copy = (when: string) => `This conversation changed ${when} and may still be open in another terminal. Resume here anyway?`;
  it("is null for a Marshell conversation", () => {
    expect(resumeWarning("marshell", now - 1000, now)).toBeNull();
  });
  it("names whole minutes for an outside conversation", () => {
    expect(resumeWarning("terminal", now - 3 * 60_000 - 20_000, now)).toBe(copy("3 min ago"));
  });
  it("says less than a minute under 60 seconds", () => {
    expect(resumeWarning("vscode", now - 30_000, now)).toBe(copy("less than a minute ago"));
  });
  it("is null from 10 minutes on", () => {
    expect(resumeWarning("terminal", now - 600_000, now)).toBeNull();
    expect(resumeWarning("terminal", now - 599_000, now)).not.toBeNull();
  });
});
