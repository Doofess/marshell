import { describe, expect, it } from "vitest";
import { NO_WORKTREE_SETUP, WORKTREE_UNREADABLE, afterCreate, readyPending, setupOutcome } from "./afterSetup";
import { infoFixture, scriptsFixture } from "./fixtures";

describe("setupOutcome", () => {
  it("is null while the setup tab runs", () => {
    expect(setupOutcome(infoFixture({ state: { kind: "running" } }))).toBeNull();
  });
  it("is Passed for exit 0", () => {
    expect(setupOutcome(infoFixture({ state: { kind: "exited", code: 0 } }))).toEqual({ kind: "passed" });
  });
  it("is Failed with the code for any other exit", () => {
    expect(setupOutcome(infoFixture({ state: { kind: "exited", code: 2 } }))).toEqual({ kind: "failed", code: 2 });
  });
  it("is Closed when the session Ended", () => {
    expect(setupOutcome(infoFixture({ state: { kind: "ended" } }))).toEqual({ kind: "closed" });
  });
  it("is Closed when the tab is gone", () => {
    expect(setupOutcome(undefined)).toEqual({ kind: "closed" });
  });
});

describe("readyPending", () => {
  const running = infoFixture({ tab_id: "s1" });
  it("waits for a tab the list has not named yet, instead of calling it closed", () => {
    const r = readyPending(new Map([["s1", { seen: false }]]), []);
    expect(r).toEqual({ seen: [], ready: [] });
  });
  it("marks a tab seen when the list names it, and waits while it runs", () => {
    expect(readyPending(new Map([["s1", { seen: false }]]), [running])).toEqual({ seen: ["s1"], ready: [] });
  });
  it("is ready with the outcome when the tab has exited", () => {
    const done = infoFixture({ tab_id: "s1", state: { kind: "exited", code: 3 } });
    expect(readyPending(new Map([["s1", { seen: true }]]), [done]).ready).toEqual([{ tabId: "s1", outcome: { kind: "failed", code: 3 } }]);
  });
  it("is ready as Closed when a tab seen before is gone", () => {
    expect(readyPending(new Map([["s1", { seen: true }]]), []).ready).toEqual([{ tabId: "s1", outcome: { kind: "closed" } }]);
  });
  it("does not touch other tabs", () => {
    const other = infoFixture({ tab_id: "other", state: { kind: "exited", code: 0 } });
    expect(readyPending(new Map([["s1", { seen: true }]]), [running, other]).ready).toEqual([]);
  });
});

describe("afterCreate (the worktree's own marshell.json, read after the task is made)", () => {
  it("runs the setup when the worktree's file is the one the person saw", () => {
    expect(afterCreate("h1", scriptsFixture({ hash: "h1", trusted: true }))).toEqual({ kind: "run", hash: "h1" });
  });
  it("runs it for a different file the person already allowed", () => {
    expect(afterCreate("h1", scriptsFixture({ hash: "h2", trusted: true }))).toEqual({ kind: "run", hash: "h2" });
  });
  it("asks again for a different file that has a setup", () => {
    const fresh = scriptsFixture({ hash: "h2", trusted: false });
    expect(afterCreate("h1", fresh)).toEqual({ kind: "ask", scripts: fresh });
  });
  it("runs nothing and says so when the worktree's file has no setup, or no file", () => {
    expect(afterCreate("h1", scriptsFixture({ setup: null }))).toEqual({ kind: "none", announce: NO_WORKTREE_SETUP });
    expect(afterCreate("h1", scriptsFixture({ setup: null, hash: null }))).toEqual({ kind: "none", announce: NO_WORKTREE_SETUP });
    expect(NO_WORKTREE_SETUP).toBe("The new worktree's marshell.json has no setup script, so none ran.");
  });
  it("runs nothing for a broken file, and says the core's own sentence", () => {
    expect(afterCreate("h1", scriptsFixture({ problem: "marshell.json: expected a value at line 1" }))).toEqual({ kind: "none", announce: "marshell.json: expected a value at line 1" });
  });
  it("runs nothing when the file could not be read", () => {
    expect(afterCreate("h1", null)).toEqual({ kind: "none", announce: WORKTREE_UNREADABLE });
  });
});
