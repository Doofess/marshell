import { describe, expect, it } from "vitest";
import { commandById, shortcutLabel } from "../../lib/keymap";
import { sessionMenuItems, type SessionMenuFacts } from "./sessionMenu";

const all: SessionMenuFacts = { resumable: true, task: true, inShownPane: false, paneAvailable: true };
const none: SessionMenuFacts = { resumable: false, task: false, inShownPane: false, paneAvailable: false };

const ids = (facts: SessionMenuFacts) =>
  sessionMenuItems(facts, "windows").map((i) => ("separator" in i ? "-" : i.id));

describe("sessionMenuItems", () => {
  it("shows every item in order with its shortcut", () => {
    const items = sessionMenuItems(all, "windows");
    expect(ids(all)).toEqual(["rename", "split", "resume", "review-task", "-", "close"]);
    expect(items.map((i) => ("separator" in i ? undefined : i.shortcut))).toEqual([
      "F2",
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
