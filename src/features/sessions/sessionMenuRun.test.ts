import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";
import { focusAfterMenu, menuFacts, modeOfItem } from "./sessionMenuRun";

const info = (tab_id: string, running = true, profile_id = "pwsh"): SessionInfo =>
  ({ tab_id, profile_id, state: running ? { kind: "running" } : { kind: "exited", code: 0 } }) as SessionInfo;
const row = (id: string, task = false) => ({ id, task }) as RowModel;
const ctx = (over: Partial<Parameters<typeof menuFacts>[1]> = {}) => ({
  infos: [info("x")],
  profiles: [{ id: "pwsh" }] as Profile[],
  rows: [row("x")],
  panes: ["x", null] as (string | null)[],
  shown: 2,
  fit: 4,
  ...over,
});

describe("menuFacts", () => {
  it("a running session is not resumable", () => {
    expect(menuFacts("x", ctx()).resumable).toBe(false);
  });
  it("an exited one is", () => {
    expect(menuFacts("x", ctx({ infos: [info("x", false)] })).resumable).toBe(true);
  });
  it("task follows the row", () => {
    expect(menuFacts("x", ctx()).task).toBe(false);
    expect(menuFacts("x", ctx({ rows: [row("x", true)] })).task).toBe(true);
  });
  it("a session in a shown pane is inShownPane; one in a pane out of sight is not", () => {
    expect(menuFacts("x", ctx()).inShownPane).toBe(true);
    expect(menuFacts("x", ctx({ panes: [null, "x"], shown: 1 })).inShownPane).toBe(false);
  });
  it("four panes with no empty one leave no room", () => {
    expect(menuFacts("z", ctx({ panes: ["a", "b", "c", "d"], shown: 4, fit: 4 })).paneAvailable).toBe(false);
  });
  it("an empty shown pane is available", () => {
    expect(menuFacts("z", ctx({ panes: ["x", null] })).paneAvailable).toBe(true);
  });
  it("room for another pane is available", () => {
    expect(menuFacts("z", ctx({ panes: ["a"], shown: 1, fit: 4 })).paneAvailable).toBe(true);
  });
});

describe("menuFacts: Resume is offered only where the Resume bar would let it run", () => {
  const exited = (profile: string, profiles: Profile[]) => menuFacts("x", ctx({ infos: [info("x", false, profile)], profiles }));
  it("is not resumable when its shell profile was removed", () => {
    expect(exited("gone", [{ id: "pwsh" }] as Profile[]).resumable).toBe(false);
  });
  it("a custom command has no profile to lose", () => {
    expect(exited("custom", [{ id: "pwsh" }] as Profile[]).resumable).toBe(true);
  });
  it("an empty profile list is unknown, not removed", () => {
    expect(exited("gone", []).resumable).toBe(true);
  });
});

describe("focusAfterMenu", () => {
  it("leaves focus to rename's box and the review sheet, which take it themselves", () => {
    expect(focusAfterMenu("rename", false)).toBeNull();
    expect(focusAfterMenu("review-task", false)).toBeNull();
  });
  it("puts focus back on the row after Resume, as Esc does", () => {
    expect(focusAfterMenu("resume", false)).toBe("row");
  });
  it("puts focus in the terminal after Split and after a Close that goes straight through", () => {
    expect(focusAfterMenu("split", false)).toBe("terminal");
    expect(focusAfterMenu("close", false)).toBe("terminal");
  });
  it("leaves it to the close bar when Close needs the bar, which focuses Cancel itself", () => {
    expect(focusAfterMenu("close", true)).toBeNull();
  });
});

describe("Mode ▸ facts and focus", () => {
  it("an agent session offers Mode, a plain shell does not", () => {
    const agent = { ...info("x"), agent: "claude" } as SessionInfo;
    expect(menuFacts("x", ctx({ infos: [agent] })).agent).toBe(true);
    expect(menuFacts("x", ctx()).agent).toBe(false);
  });

  it("a shell promoted to an agent by its foreground has no Mode", () => {
    const promoted = { ...info("x"), agent: null, foreground: { program: "claude", command_line: "claude", agent: "claude" } } as SessionInfo;
    expect(menuFacts("x", ctx({ infos: [promoted] })).agent).toBe(false);
  });
  it("the Mode item and a picked mode leave focus to the menu or the bar", () => {
    expect(focusAfterMenu("mode", false)).toBeNull();
    expect(focusAfterMenu("mode-plan", false)).toBeNull();
  });
  it("modeOfItem reads a mode from a menu item or palette command id", () => {
    expect(modeOfItem("mode-auto-edit")).toBe("auto-edit");
    expect(modeOfItem("mode-bypass")).toBe("bypass");
    expect(modeOfItem("mode")).toBeNull();
    expect(modeOfItem("mode-nope")).toBeNull();
    expect(modeOfItem("rename")).toBeNull();
  });
});

describe("Bookmark fact", () => {
  it("is true for a Claude Code tab only", () => {
    const claude = { ...info("x"), agent: "claude" } as SessionInfo;
    const codex = { ...info("x"), agent: "codex" } as SessionInfo;
    expect(menuFacts("x", ctx({ infos: [claude] })).bookmarkable).toBe(true);
    expect(menuFacts("x", ctx({ infos: [codex] })).bookmarkable).toBe(false);
    expect(menuFacts("x", ctx()).bookmarkable).toBe(false);
  });
  it("leaves focus on the row, where the menu was", () => {
    expect(focusAfterMenu("bookmark", false)).toBe("row");
  });
});
