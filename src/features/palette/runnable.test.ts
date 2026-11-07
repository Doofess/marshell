import { describe, expect, it } from "vitest";
import { cannotRun, type RunFacts } from "./runnable";

const ALL: RunFacts = { focused: true, sessions: 2, split: true, waiting: true, closed: true, shellPanel: true, sidebarToggle: true };
const sorted = (xs: readonly string[]) => [...xs].sort();

describe("cannotRun", () => {
  it("each_missing_fact_leaves_out_its_commands", () => {
    expect(cannotRun(ALL)).toEqual([]);
    expect(sorted(cannotRun({ ...ALL, focused: false }))).toEqual(sorted(["close", "rename", "focus-prompt", "find", "bookmark", "announce-status", "prompt-prev", "prompt-next", "move-up", "move-down"]));
    expect(cannotRun({ ...ALL, sessions: 0 })).toContain("go-to-session");
    expect(sorted(cannotRun({ ...ALL, sessions: 1 }))).toEqual(["move-down", "move-up"]);
    expect(sorted(cannotRun({ ...ALL, split: false }))).toEqual(["layout", "unsplit", "zoom-pane"]);
    expect(cannotRun({ ...ALL, waiting: false })).toEqual(["next-waiting"]);
    expect(cannotRun({ ...ALL, closed: false })).toEqual(["reopen"]);
    expect(cannotRun({ ...ALL, shellPanel: false })).toEqual(["shell-panel"]);
    expect(cannotRun({ ...ALL, sidebarToggle: false })).toEqual(["sidebar"]);
  });

  it("lists each command once when several facts are missing", () => {
    const out = cannotRun({ focused: false, sessions: 0, split: false, waiting: false, closed: false, shellPanel: false, sidebarToggle: false });
    expect(new Set(out).size).toBe(out.length);
    expect(out).toContain("move-up");
  });
});
