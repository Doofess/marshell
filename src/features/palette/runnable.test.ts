import { describe, expect, it } from "vitest";
import { cannotRun, type RunFacts } from "./runnable";

const ALL: RunFacts = { focused: true, sessions: 2, waiting: true, closed: true, shellPanel: true, sidebarToggle: true, textTab: true, undoSave: true };
const sorted = (xs: readonly string[]) => [...xs].sort();

describe("cannotRun", () => {
  it("each_missing_fact_leaves_out_its_commands", () => {
    expect(cannotRun(ALL)).toEqual([]);
    expect(sorted(cannotRun({ ...ALL, focused: false }))).toEqual(sorted(["close", "rename", "focus-prompt", "find", "bookmark", "announce-status", "prompt-prev", "prompt-next", "move-up", "move-down"]));
    expect(cannotRun({ ...ALL, sessions: 0 })).toContain("go-to-session");
    expect(sorted(cannotRun({ ...ALL, sessions: 1 }))).toEqual(["move-down", "move-up"]);
    expect(cannotRun({ ...ALL, waiting: false })).toEqual(["next-waiting"]);
    expect(cannotRun({ ...ALL, closed: false })).toEqual(["reopen"]);
    expect(cannotRun({ ...ALL, shellPanel: false })).toEqual(["shell-panel"]);
    expect(cannotRun({ ...ALL, sidebarToggle: false })).toEqual(["sidebar"]);
  });

  it("the_four_file_commands_show_only_for_a_text_tab", () => {
    const four = ["pane-save", "pane-undo-save", "pane-go-to-line", "pane-word-wrap"];
    expect(cannotRun(ALL)).toEqual([]);
    // No text tab on screen: all four go, whether or not an undo would be possible.
    expect(sorted(cannotRun({ ...ALL, textTab: false }))).toEqual(sorted(four));
    expect(sorted(cannotRun({ ...ALL, textTab: false, undoSave: false }))).toEqual(sorted(four));
    // A text tab with no save to undo leaves out only the undo.
    expect(cannotRun({ ...ALL, undoSave: false })).toEqual(["pane-undo-save"]);
  });

  it("lists each command once when several facts are missing", () => {
    const out = cannotRun({ focused: false, sessions: 0, waiting: false, closed: false, shellPanel: false, sidebarToggle: false, textTab: false, undoSave: false });
    expect(new Set(out).size).toBe(out.length);
    expect(out).toContain("move-up");
  });
});
