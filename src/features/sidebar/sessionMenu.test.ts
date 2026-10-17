import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import { modeMenuItems, sessionMenuItems, type SessionMenuFacts } from "./sessionMenu";

const all: SessionMenuFacts = { agent: true, resumable: true, task: true, inShownPane: false, paneAvailable: true, pausable: true, paused: false };
const none: SessionMenuFacts = { agent: false, resumable: false, task: false, inShownPane: false, paneAvailable: false, pausable: false, paused: false };

const ids = (facts: SessionMenuFacts) =>
  sessionMenuItems(facts, "windows").map((i) => ("separator" in i ? "-" : i.id));

describe("sessionMenuItems", () => {
  it("shows every item in order with its shortcut", () => {
    const items = sessionMenuItems(all, "windows");
    expect(ids(all)).toEqual(["rename", "split", "resume", "mode", "pause", "review-task", "-", "close"]);
    expect(items.map((i) => ("separator" in i ? undefined : i.shortcut))).toEqual([
      "F2",
      undefined,
      undefined,
      undefined,
      undefined,
      "Ctrl+Shift+R",
      undefined,
      "Ctrl+Shift+W",
    ]);
  });
  it("hides Split for a session already in a shown pane", () => {
    expect(ids({ ...all, inShownPane: true })).not.toContain("split");
  });
  it("hides Split when no pane is available", () => {
    expect(ids({ ...all, paneAvailable: false })).not.toContain("split");
  });
  it("hides Resume when the session is not resumable", () => {
    expect(ids({ ...all, resumable: false })).not.toContain("resume");
  });
  it("hides Review changes without a task", () => {
    expect(ids({ ...all, task: false })).not.toContain("review-task");
  });
  it("keeps only Rename and Close with minimal facts", () => {
    expect(ids(none)).toEqual(["rename", "-", "close"]);
  });
  it("uses the platform's close shortcut", () => {
    const close = sessionMenuItems(all, "mac").find((i) => "id" in i && i.id === "close");
    expect(close).toMatchObject({ shortcut: shortcutLabel(commandById("close").binding!, "mac") });
  });
});

describe("Mode ▸", () => {
  it("is offered for an agent session and not for a plain shell", () => {
    const labels = sessionMenuItems(all, "windows").map((i) => ("separator" in i ? "-" : i.label));
    expect(labels).toContain("Mode ▸");
    expect(ids({ ...all, agent: false })).not.toContain("mode");
  });
  it("modeMenuItems lists the five modes and marks the one it started in", () => {
    const items = modeMenuItems("plan");
    expect(items.map((i) => ("separator" in i ? "-" : i.id))).toEqual(["mode-manual", "mode-plan", "mode-auto-edit", "mode-full-auto", "mode-bypass"]);
    expect(items.map((i) => ("separator" in i ? "-" : i.label))).toEqual(["Manual", "Plan (current)", "Auto-edit", "Full auto", "Bypass"]);
  });
  it("marks none as current when the mode is unknown", () => {
    expect(modeMenuItems(undefined).map((i) => ("separator" in i ? "-" : i.label)).join()).not.toContain("current");
  });
});

describe("Pause and Continue", () => {
  it("a running session lists Pause and not Continue", () => {
    expect(ids(all)).toContain("pause");
    expect(ids(all)).not.toContain("continue");
  });
  it("a paused session lists Continue and not Pause", () => {
    const paused = { ...all, pausable: false, paused: true };
    expect(ids(paused)).toContain("continue");
    expect(ids(paused)).not.toContain("pause");
  });
  it("an ended session lists neither", () => {
    expect(ids(none)).not.toContain("pause");
    expect(ids(none)).not.toContain("continue");
  });
  it("they sit after Mode and before Review changes", () => {
    const list = ids({ ...all, paused: true, pausable: false });
    expect(list.indexOf("continue")).toBe(list.indexOf("mode") + 1);
    expect(list.indexOf("continue")).toBeLessThan(list.indexOf("review-task"));
  });
});
