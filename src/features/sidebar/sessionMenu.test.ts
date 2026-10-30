import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import type { SessionInfo } from "../../generated/SessionInfo";
import { menuFacts } from "../sessions/sessionMenuRun";
import { FIXTURES } from "./fixtures";
import { modeMenuItems, sessionMenuItems, type SessionMenuFacts } from "./sessionMenu";

const all: SessionMenuFacts = { agent: true, modeChangeable: true, resumable: true, task: true, inShownPane: false, paneAvailable: true, pausable: true, paused: false, muted: false, bookmarkable: true, setupOutput: false };
const none: SessionMenuFacts = { agent: false, modeChangeable: true, resumable: false, task: false, inShownPane: false, paneAvailable: false, pausable: false, paused: false, muted: false, bookmarkable: false, setupOutput: false };

const ids = (facts: SessionMenuFacts) =>
  sessionMenuItems(facts, "windows").map((i) => ("separator" in i ? "-" : i.id));

describe("sessionMenuItems", () => {
  it("shows every item in order with its shortcut", () => {
    const items = sessionMenuItems(all, "windows");
    expect(ids(all)).toEqual(["rename", "split", "resume", "copy-resume", "open-outside", "mode", "pause", "bookmark", "mute", "review-task", "-", "close"]);
    expect(items.map((i) => ("separator" in i ? undefined : i.shortcut))).toEqual([
      "F2",
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      "Ctrl+Shift+B",
      undefined,
      "Ctrl+Shift+R",
      undefined,
      "Ctrl+Shift+W",
    ]);
  });
  it("agent_sessions_get_both_items: after Resume and before Mode, running or not, and a plain shell has neither", () => {
    for (const facts of [all, { ...all, resumable: false, pausable: false }]) {
      const got = ids(facts);
      expect(got.indexOf("copy-resume")).toBeGreaterThan(-1);
      expect(got.indexOf("open-outside")).toBe(got.indexOf("copy-resume") + 1);
      expect(got.indexOf("mode")).toBe(got.indexOf("open-outside") + 1);
    }
    expect(ids({ ...all, modeChangeable: false })).toEqual(expect.arrayContaining(["copy-resume", "open-outside"]));
    expect(ids(none)).not.toContain("copy-resume");
    expect(ids(none)).not.toContain("open-outside");
    const labels = sessionMenuItems(all, "windows").flatMap((i) => ("separator" in i ? [] : [i.label]));
    expect(labels).toEqual(expect.arrayContaining(["Copy resume command", "Open in system terminal"]));
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
  it("keeps only Rename, Mute and Close with minimal facts", () => {
    expect(ids(none)).toEqual(["rename", "mute", "-", "close"]);
  });
  it("uses the platform's close shortcut", () => {
    const close = sessionMenuItems(all, "mac").find((i) => "id" in i && i.id === "close");
    expect(close).toMatchObject({ shortcut: shortcutLabel(commandById("close").binding!, "mac") });
  });
});

describe("Mode ▸", () => {
  it("hides Mode for an ended agent in a running shell, then restores it when the shell exits", () => {
    const row = { ...FIXTURES[0]!, status: "ended" as const };
    const info = { tab_id: row.id, agent: "claude", state: { kind: "running" }, paused: false } as SessionInfo;
    const ctx = { infos: [info], rows: [row], profiles: [], panes: [row.id], shown: 1, fit: 4 };
    expect(ids(menuFacts(row.id, ctx))).not.toContain("mode");
    expect(ids(menuFacts(row.id, { ...ctx, infos: [{ ...info, state: { kind: "exited", code: 0 } }] }))).toContain("mode");
    expect(ids(menuFacts(row.id, { ...ctx, rows: [{ ...row, status: "working" }] }))).toContain("mode");
  });
  it("is offered for an agent session and not for a plain shell", () => {
    const labels = sessionMenuItems(all, "windows").map((i) => ("separator" in i ? "-" : i.label));
    expect(labels).toContain("Mode ▸");
    expect(ids({ ...all, agent: false })).not.toContain("mode");
  });
  it("modeMenuItems never offers Don't ask", () => {
    expect(modeMenuItems("dont-ask").map((i) => ("separator" in i ? "-" : i.id))).not.toContain("mode-dont-ask");
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

describe("Mute notifications", () => {
  const labelOf = (facts: SessionMenuFacts, id: string) => {
    const item = sessionMenuItems(facts, "windows").find((i) => "id" in i && i.id === id);
    return item && "label" in item ? item.label : undefined;
  };
  it("a session that is not muted lists Mute notifications and not Unmute", () => {
    expect(labelOf(all, "mute")).toBe("Mute notifications");
    expect(ids(all)).not.toContain("unmute");
  });
  it("a muted session lists Unmute notifications and not Mute", () => {
    expect(labelOf({ ...all, muted: true }, "unmute")).toBe("Unmute notifications");
    expect(ids({ ...all, muted: true })).not.toContain("mute");
  });
  it("an ended session offers it too", () => {
    expect(ids(none)).toContain("mute");
  });
});

describe("Bookmark", () => {
  it("the row menu shows Bookmark for a Claude tab only", () => {
    expect(ids(all)).toContain("bookmark");
    expect(ids({ ...all, bookmarkable: false })).not.toContain("bookmark");
    expect(ids(none)).not.toContain("bookmark");
  });
  it("it carries the key it runs on and sits after Pause and before Mute", () => {
    const items = sessionMenuItems(all, "windows");
    const item = items.find((i) => "id" in i && i.id === "bookmark");
    expect(item && "label" in item ? [item.label, item.shortcut] : null).toEqual(["Bookmark", "Ctrl+Shift+B"]);
    const list = ids(all);
    expect(list.indexOf("bookmark")).toBeGreaterThan(list.indexOf("pause"));
    expect(list.indexOf("bookmark")).toBeLessThan(list.indexOf("mute"));
  });
});

describe("modeMenuItems for an agent that maps only some modes", () => {
  it("lists only the offered ones, in MODES order", () => {
    const items = modeMenuItems("manual", ["manual", "plan", "bypass"]);
    expect(items.map((i) => ("separator" in i ? "-" : i.id))).toEqual(["mode-manual", "mode-plan", "mode-bypass"]);
  });
});

describe("Open setup output", () => {
  it("is offered only when the row has a setup tab to open, just before Review changes", () => {
    expect(ids(all)).not.toContain("setup-output");
    const list = ids({ ...all, setupOutput: true });
    expect(list).toContain("setup-output");
    expect(list.indexOf("setup-output")).toBe(list.indexOf("review-task") - 1);
    const item = sessionMenuItems({ ...none, setupOutput: true }, "windows").find((i) => "id" in i && i.id === "setup-output");
    expect(item).toMatchObject({ label: "Open setup output" });
    expect(ids({ ...none, setupOutput: true })).toContain("setup-output");
  });
});
