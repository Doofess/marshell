import { describe, expect, it } from "vitest";
import type { PermissionView } from "../../generated/PermissionView";
import type { RowModel } from "../sidebar/types";
import { decisionFor, laneEntries, toDetail, toRequest } from "./fromCore";

const NOW = 1_000_000;
const row = (over: Partial<RowModel> = {}): RowModel => ({ id: "tab-a", number: 3, name: "infra", project: "infra", projectKey: "infra", agent: "claude", status: "working", phrase: "", ...over });
const view = (over: Partial<PermissionView> = {}): PermissionView => ({
  rid: "r1",
  tab: "tab-a",
  agent: "claude",
  at_ms: NOW - 65_000,
  detail: { kind: "bash", command: "npm test", cwd: null },
  risk: null,
  always_rule: "Bash(npm test:*)",
  ...over,
});

describe("toRequest", () => {
  it("maps the wait, the session, the rule and the tab", () => {
    const r = toRequest(view(), row(), NOW);
    expect(r).toMatchObject({ id: "r1", tab: "tab-a", waitingMs: 65_000, alwaysRule: "Bash(npm test:*)", session: { name: "infra", agent: "claude", number: 3 }, state: { phase: "pending" }, risk: { level: "normal" } });
    expect(r.terminalOnly).toBeUndefined();
  });
  it("maps a risk reason to a risky request, and no rule to none", () => {
    const r = toRequest(view({ risk: "Deletes files recursively", always_rule: null }), row(), NOW);
    expect(r.risk).toEqual({ level: "risky", reason: "Deletes files recursively" });
    expect(r.alwaysRule).toBeUndefined();
  });
  it("still maps a request whose session row is not listed yet", () => {
    const r = toRequest(view(), undefined, NOW);
    expect(r.session.name).toBe("tab-a");
    expect(r.session.agent).toBe("claude");
  });
  it("never shows a negative wait", () => {
    expect(toRequest(view({ at_ms: NOW + 5000 }), row(), NOW).waitingMs).toBe(0);
  });
});

describe("toDetail", () => {
  it("maps each kind, turning pairs into records and null into absent", () => {
    expect(toDetail({ kind: "bash", command: "ls", cwd: "C:/x" })).toEqual({ kind: "bash", command: "ls", cwd: "C:/x" });
    expect(toDetail({ kind: "bash", command: "ls", cwd: null })).toEqual({ kind: "bash", command: "ls", cwd: undefined });
    expect(toDetail({ kind: "write", path: "a", preview: "x", is_new: true })).toEqual({ kind: "write", path: "a", preview: "x", isNew: true });
    expect(toDetail({ kind: "mcp", server: "gh", tool: "pr", args: [["a", "1"]] })).toEqual({ kind: "mcp", server: "gh", tool: "pr", args: { a: "1" } });
  });
  it("maps an unknown tool to the other card with its input as key: value lines", () => {
    expect(toDetail({ kind: "other", tool: "NotebookEdit", args: [["cell", "3"]] })).toEqual({ kind: "other", tool: "NotebookEdit", args: { cell: "3" } });
  });
});

describe("laneEntries", () => {
  it("does not add a terminal fallback while a vanished card is sending", () => {
    expect(laneEntries([], [row({ status: "needs-permission" })], NOW, new Set(["tab-a"]))).toEqual([]);
  });
  it("adds a terminal-only entry for a needs-you row with no view", () => {
    const e = laneEntries([], [row({ status: "needs-question", phrase: "Pick a region", waitingMs: 9000 })], NOW);
    expect(e).toHaveLength(1);
    expect(e[0]).toMatchObject({ tab: "tab-a", terminalOnly: true, detail: { kind: "question", question: "Pick a region", options: [] }, waitingMs: 9000 });
  });
  it("does not count a row that has a view twice", () => {
    const e = laneEntries([view()], [row({ status: "needs-permission" })], NOW);
    expect(e).toHaveLength(1);
    expect(e[0]!.terminalOnly).toBeUndefined();
  });
  it("ignores rows that are not waiting", () => {
    expect(laneEntries([], [row({ status: "working" }), row({ id: "b", status: "idle" })], NOW)).toEqual([]);
  });
  it("sorts oldest first", () => {
    const e = laneEntries(
      [view({ rid: "new", at_ms: NOW - 1000 }), view({ rid: "old", at_ms: NOW - 50_000, tab: "tab-b" })],
      [row(), row({ id: "tab-b", name: "web" }), row({ id: "tab-c", name: "q", status: "needs-question", waitingMs: 20_000 })],
      NOW,
    );
    expect(e.map((x) => x.id)).toEqual(["old", "terminal-tab-c", "new"]);
  });
  it("gives two views on one tab two entries", () => {
    const e = laneEntries([view({ rid: "a", at_ms: NOW - 3000 }), view({ rid: "b", at_ms: NOW - 2000 })], [row({ status: "needs-permission" })], NOW);
    expect(e.map((x) => x.id)).toEqual(["a", "b"]);
  });
});

describe("decisionFor", () => {
  it("allows once, allows for the project, and denies", () => {
    expect(decisionFor("allow")).toEqual({ behavior: "allow", scope: "once", message: null });
    expect(decisionFor("always")).toEqual({ behavior: "allow", scope: "project", message: null });
    expect(decisionFor("deny")).toMatchObject({ behavior: "deny" });
  });
});
