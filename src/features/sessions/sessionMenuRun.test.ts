import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";
import { focusAfterMenu, menuFacts, modeOfItem, runResumeItem } from "./sessionMenuRun";
import { toastFor } from "./resumeActions";

const info = (tab_id: string, running = true, profile_id = "pwsh"): SessionInfo =>
  ({ tab_id, profile_id, state: running ? { kind: "running" } : { kind: "exited", code: 0 } }) as SessionInfo;
const row = (id: string, task = false, setupTab?: string) => ({ id, task, ...(setupTab ? { setupNote: { text: "Setup failed (exit 2).", tabId: setupTab } } : {}) }) as RowModel;
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

describe("setupOutput fact", () => {
  it("is true only when the row's setup note still has its tab", () => {
    expect(menuFacts("x", ctx()).setupOutput).toBe(false);
    expect(menuFacts("x", ctx({ rows: [row("x", false, "t-s")] })).setupOutput).toBe(true);
    expect(menuFacts("x", ctx({ rows: [{ id: "x", setupNote: { text: "Setup was closed before it finished." } } as RowModel] })).setupOutput).toBe(false);
  });
  it("leaves focus to the setup tab it selects", () => {
    expect(focusAfterMenu("setup-output", false)).toBeNull();
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

describe("menuFacts for an agent's modes", () => {
  const agentInfo = (mode: string) => ({ ...info("x"), agent: "cursor", mode }) as SessionInfo;
  it("Mode ▸ is left out when the agent offers no other mode", () => {
    expect(menuFacts("x", ctx({ infos: [agentInfo("manual")], offered: () => [] })).modeChangeable).toBe(false);
    expect(menuFacts("x", ctx({ infos: [agentInfo("manual")], offered: () => ["manual"] })).modeChangeable).toBe(false);
  });
  it("Mode ▸ stays when another mode is offered", () => {
    expect(menuFacts("x", ctx({ infos: [agentInfo("manual")], offered: () => ["manual", "plan"] })).modeChangeable).toBe(true);
  });
});

describe("runResumeItem", () => {
  const endpoint = { port: 9, token: "t", platform: "windows" } as Endpoint;
  const setup = () => {
    const api = {
      resumeCommand: vi.fn().mockResolvedValue({ ok: true, command: "cd x; claude --resume 1", form: "by_id" }),
      openOutside: vi.fn().mockResolvedValue({ ok: true }),
    };
    const announce = vi.fn();
    const write = vi.fn().mockResolvedValue(undefined);
    return { api, announce, write, deps: { endpoint, announce, write, api } };
  };
  it("open_outside_on_a_running_session_toasts and calls no API", async () => {
    const s = setup();
    await runResumeItem("open-outside", { tabId: "t1", running: true }, s.deps);
    expect(s.announce).toHaveBeenCalledWith("End the session first.");
    expect(s.api.openOutside).not.toHaveBeenCalled();
  });
  it("open-outside on an ended session asks the core and says so", async () => {
    const s = setup();
    await runResumeItem("open-outside", { tabId: "t1", running: false }, s.deps);
    expect(s.api.openOutside).toHaveBeenCalledWith(endpoint, "t1");
    expect(s.announce).toHaveBeenCalledWith("Opened in a system terminal.");
  });
  it("copy works on a running session: copies the command and toasts its form", async () => {
    const s = setup();
    await runResumeItem("copy-resume", { tabId: "t1", running: true }, s.deps);
    expect(s.write).toHaveBeenCalledWith("cd x; claude --resume 1");
    expect(s.announce).toHaveBeenCalledWith("Copied the resume command.");
  });
  it("a refused copy shows the core's reason and copies nothing", async () => {
    const s = setup();
    s.api.resumeCommand.mockResolvedValue({ ok: false, reason: "Codex has no resume command." });
    await runResumeItem("copy-resume", { tabId: "t1", running: false }, s.deps);
    expect(s.announce).toHaveBeenCalledWith("Codex has no resume command.");
    expect(s.write).not.toHaveBeenCalled();
  });
  it("a refused open shows the core's reason", async () => {
    const s = setup();
    s.api.openOutside.mockResolvedValue({ ok: false, reason: "Open in system terminal works on Windows only for now." });
    await runResumeItem("open-outside", { tabId: "t1", running: false }, s.deps);
    expect(s.announce).toHaveBeenCalledWith("Open in system terminal works on Windows only for now.");
  });
  it("a failed request is announced, not thrown", async () => {
    const s = setup();
    s.api.resumeCommand.mockRejectedValue(new Error("down"));
    await runResumeItem("copy-resume", { tabId: "t1", running: false }, s.deps);
    expect(s.announce).toHaveBeenCalledWith("Couldn't copy the resume command: down");
  });
});

describe("toastFor", () => {
  it("toastFor_maps_forms: by_id", () => expect(toastFor("by_id")).toBe("Copied the resume command."));
  it("toastFor_maps_forms: continue_last", () => expect(toastFor("continue_last")).toBe("Copied. It continues the last conversation in this folder."));
  it("toastFor_maps_forms: fresh", () => expect(toastFor("fresh")).toBe("Copied. It starts a new conversation: this one was never saved."));
});
