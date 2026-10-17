import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { SessionActions, restartBarText } from "./sessionActions";

const endpoint = { port: 1, token: "t", platform: "windows" } as Endpoint;
const info = (tab_id: string): SessionInfo => ({ tab_id, number: 1, name: tab_id, agent: null, profile_id: "pwsh", cwd: "C:/x", project: "C:/x", state: { kind: "running" } }) as SessionInfo;

function setup(selectedId: string | undefined) {
  const close = vi.fn().mockResolvedValue(undefined);
  const actions = new SessionActions({
    api: { close, reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn() },
    context: () => ({ endpoint, infos: [info("a"), info("b")], selectedId }),
    select: vi.fn(),
  });
  return { actions, close };
}

describe("an explicit tab id", () => {
  it("requestClose closes that session, not the selected one", async () => {
    const { actions, close } = setup("a");
    await actions.requestClose("b");
    expect(close).toHaveBeenCalledWith(endpoint, "b");
  });
  it("startRename renames that session", () => {
    const { actions } = setup("a");
    actions.startRename("b");
    expect(actions.getState().renaming).toBe("b");
  });
  it("a call with no id still acts on the selected session", async () => {
    const { actions, close } = setup("a");
    actions.startRename();
    expect(actions.getState().renaming).toBe("a");
    await actions.requestClose();
    expect(close).toHaveBeenCalledWith(endpoint, "a");
  });
  it("an id that is not a session does nothing", async () => {
    const { actions, close } = setup("a");
    actions.startRename("gone");
    await actions.requestClose("gone");
    expect(actions.getState().renaming).toBeNull();
    expect(close).not.toHaveBeenCalled();
  });
});

describe("restart in another mode", () => {
  const agentInfo = { ...info("a"), number: 3, name: "billing", agent: "claude", mode: "plan" } as SessionInfo;
  function restartSetup(restart = vi.fn().mockResolvedValue({ ok: true })) {
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart },
      context: () => ({ endpoint, infos: [agentInfo, info("b")], selectedId: "a" }),
      select: vi.fn(),
      size: () => ({ cols: 100, rows: 40 }),
    });
    return { actions, restart };
  }
  it("askRestart puts the bar up for an agent session and refuses a shell, an unknown tab and the mode it is in", () => {
    const { actions } = restartSetup();
    expect(actions.askRestart("b", "bypass")).toBe(false);
    expect(actions.askRestart("gone", "bypass")).toBe(false);
    expect(actions.askRestart("a", "plan")).toBe(false);
    expect(actions.getState().restarting).toBeNull();
    expect(actions.askRestart("a", "bypass")).toBe(true);
    expect(actions.getState().restarting).toEqual({ id: "a", mode: "bypass" });
  });
  it("confirmRestart sends the mode and size, bumps resumed and hands the focus back", async () => {
    const { actions, restart } = restartSetup();
    actions.askRestart("a", "bypass");
    const before = actions.getState().focusToken;
    await actions.confirmRestart();
    expect(restart).toHaveBeenCalledWith(endpoint, "a", "bypass", { cols: 100, rows: 40 });
    expect(actions.getState().resumed).toEqual({ a: 1 });
    expect(actions.getState().restarting).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
  });
  it("a refusal shows the core's reason in the toast and leaves resumed alone", async () => {
    const { actions } = restartSetup(vi.fn().mockResolvedValue({ ok: false, reason: "It is already starting." }));
    actions.askRestart("a", "bypass");
    await actions.confirmRestart();
    expect(actions.getState().toast?.text).toBe("It is already starting.");
    expect(actions.getState().resumed).toEqual({});
  });
  it("a failed request says so", async () => {
    const { actions } = restartSetup(vi.fn().mockRejectedValue(new Error("down")));
    actions.askRestart("a", "bypass");
    await actions.confirmRestart();
    expect(actions.getState().toast?.text).toContain("Couldn't restart the session");
    expect(actions.getState().resumed).toEqual({});
  });
  it("cancelRestart closes the bar, sends nothing and returns the focus", () => {
    const { actions, restart } = restartSetup();
    actions.askRestart("a", "bypass");
    const before = actions.getState().focusToken;
    actions.cancelRestart();
    expect(actions.getState().restarting).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
    expect(restart).not.toHaveBeenCalled();
  });
  it("confirmRestart with no bar up does nothing", async () => {
    const { actions, restart } = restartSetup();
    await actions.confirmRestart();
    expect(restart).not.toHaveBeenCalled();
  });
  it("restartBarText names the session, the mode and the command, and warns when it is working", () => {
    expect(restartBarText(agentInfo, "bypass", false)).toBe("Restart 3 billing in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions.");
    expect(restartBarText(agentInfo, "bypass", true)).toBe("Restart 3 billing in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions. Claude Code may be mid-task.");
  });
});
