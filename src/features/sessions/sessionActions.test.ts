import { describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { ApiError } from "../../lib/api/client";
import { BYPASS_ACK_TEXT, PAUSE_UNAVAILABLE, SessionActions, needsBypassAck, pauseBarStale, pauseBarText, reopenBarText, restartBarStale, restartBarText } from "./sessionActions";

const endpoint = { port: 1, token: "t", platform: "windows" } as Endpoint;
const info = (tab_id: string): SessionInfo => ({ tab_id, number: 1, name: tab_id, agent: null, profile_id: "pwsh", cwd: "C:/x", project: "C:/x", state: { kind: "running" } }) as SessionInfo;

describe("reopenBarText", () => {
  const row = { ...info("closed"), number: 3, name: "billing", agent: "antigravity", elevated: true } as SessionInfo;
  const detection = { cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", skipsAll: { setting: "dangerouslySkipPermissions" } } as const;

  it("names the closed row and the setting that makes it Bypass", () => {
    expect(reopenBarText(row, detection)).toBe("Reopen 3 billing as administrator? Antigravity's own dangerouslySkipPermissions setting skips every prompt, so it runs in Bypass.");
  });
  it("explains a pending settings read", () => {
    expect(reopenBarText(row)).toBe("Reopen 3 billing as administrator? Marshell couldn't read Antigravity's settings, so it may skip every prompt.");
  });
  it("explains a failed read even when an older detection says the setting is on", () => {
    expect(reopenBarText(row, detection, true)).toBe("Reopen 3 billing as administrator? Marshell couldn't read Antigravity's settings, so it may skip every prompt.");
  });
  it("keeps the conservative copy when the closed row is unknown", () => {
    expect(reopenBarText(undefined, detection)).toBe("Reopen the closed session as administrator? Antigravity may skip every prompt, so this session may run in Bypass.");
  });
});

function setup(selectedId: string | undefined) {
  const close = vi.fn().mockResolvedValue(undefined);
  const actions = new SessionActions({
    api: { close, reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
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
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart, pause: vi.fn(), cont: vi.fn() },
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

describe("restartBarStale", () => {
  const ids = ["a", "b"];
  it("keeps the bar while its off-screen session is still being shown, before it is focused", () => {
    expect(restartBarStale({ id: "b", closing: null, focusedId: "a", ids, showing: "b" })).toBe(false);
  });
  it("keeps the bar on the focused session", () => {
    expect(restartBarStale({ id: "b", closing: null, focusedId: "b", ids, showing: null })).toBe(false);
  });
  it("drops the bar when the focus moved to another session", () => {
    expect(restartBarStale({ id: "b", closing: null, focusedId: "a", ids, showing: null })).toBe(true);
  });
  it("drops the bar when the session is gone or a close question is up", () => {
    expect(restartBarStale({ id: "c", closing: null, focusedId: "c", ids, showing: "c" })).toBe(true);
    expect(restartBarStale({ id: "b", closing: "b", focusedId: "b", ids, showing: null })).toBe(true);
  });
});

describe("pause and continue", () => {
  const running = { ...info("a"), number: 3, name: "billing", paused: false } as SessionInfo;
  const frozen = { ...info("p"), number: 4, name: "api", paused: true } as SessionInfo;
  const ended = { ...info("e"), state: { kind: "ended" }, paused: false } as SessionInfo;
  function pauseSetup(warned: boolean, pause = vi.fn().mockResolvedValue({ ok: true })) {
    const cont = vi.fn().mockResolvedValue({ ok: true });
    const markPauseWarned = vi.fn().mockResolvedValue(undefined);
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause, cont },
      context: () => ({ endpoint, infos: [running, frozen, ended], selectedId: "a" }),
      select: vi.fn(),
      settings: { pauseWarned: warned, markPauseWarned },
    });
    return { actions, pause, cont, markPauseWarned };
  }
  const CONFIRM = "Pause 3 billing? It freezes the program and everything it started. Network connections can time out while it's paused.";

  it("the first pause sends nothing and puts the confirm bar up with the exact copy", () => {
    const { actions, pause } = pauseSetup(false);
    expect(actions.pause("a")).toBe("asked");
    expect(pause).not.toHaveBeenCalled();
    expect(actions.getState().pauseBar).toEqual({ id: "a", kind: "confirm" });
    expect(pauseBarText(running)).toBe(CONFIRM);
  });
  it("confirmPause saves pause_warned first, then calls the API, and hands the focus back", async () => {
    const { actions, pause, markPauseWarned } = pauseSetup(false);
    actions.pause("a");
    const before = actions.getState().focusToken;
    await actions.confirmPause();
    expect(markPauseWarned).toHaveBeenCalledTimes(1);
    expect(pause).toHaveBeenCalledWith(endpoint, "a");
    expect(markPauseWarned.mock.invocationCallOrder[0]).toBeLessThan(pause.mock.invocationCallOrder[0]!);
    expect(actions.getState().pauseBar).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
  });
  it("a settings write that fails still pauses: the person confirmed", async () => {
    const { actions, pause, markPauseWarned } = pauseSetup(false);
    markPauseWarned.mockRejectedValue(new Error("down"));
    actions.pause("a");
    await actions.confirmPause();
    expect(pause).toHaveBeenCalledWith(endpoint, "a");
  });
  it("pause after the warning was shown calls the API at once", async () => {
    const { actions, pause } = pauseSetup(true);
    expect(actions.pause("a")).toBe("sent");
    await Promise.resolve();
    expect(pause).toHaveBeenCalledWith(endpoint, "a");
    expect(actions.getState().pauseBar).toBeNull();
  });
  it("pause does nothing for a paused, an ended or an unknown session", () => {
    const { actions, pause } = pauseSetup(true);
    expect(actions.pause("p")).toBeNull();
    expect(actions.pause("e")).toBeNull();
    expect(actions.pause("gone")).toBeNull();
    expect(pause).not.toHaveBeenCalled();
  });
  it("a 409 shows the core's reason in the toast", async () => {
    const { actions } = pauseSetup(true, vi.fn().mockResolvedValue({ ok: false, reason: "Only a running session can be paused." }));
    actions.pause("a");
    await vi.waitFor(() => expect(actions.getState().toast?.text).toBe("Couldn't pause the session: Only a running session can be paused."));
  });
  it("cont calls the API for a paused session and shows a refusal", async () => {
    const { actions, cont } = pauseSetup(true);
    await actions.cont("p");
    expect(cont).toHaveBeenCalledWith(endpoint, "p");
    cont.mockResolvedValue({ ok: false, reason: "No." });
    await actions.cont("p");
    expect(actions.getState().toast?.text).toBe("Couldn't continue the session: No.");
  });
  it("cont does nothing for a session that is not paused", async () => {
    const { actions, cont } = pauseSetup(true);
    await actions.cont("a");
    expect(cont).not.toHaveBeenCalled();
  });
  it("showPausedBar puts the typing bar up for a paused session only, and Continue on it calls the API", async () => {
    const { actions, cont } = pauseSetup(true);
    actions.showPausedBar("a");
    expect(actions.getState().pauseBar).toBeNull();
    actions.showPausedBar("p");
    expect(actions.getState().pauseBar).toEqual({ id: "p", kind: "typing" });
    await actions.continueFromBar();
    expect(cont).toHaveBeenCalledWith(endpoint, "p");
  });
  it("cancelPause closes the bar, sends nothing, and returns the focus", () => {
    const { actions, pause } = pauseSetup(false);
    actions.pause("a");
    const before = actions.getState().focusToken;
    actions.cancelPause();
    expect(actions.getState().pauseBar).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
    expect(pause).not.toHaveBeenCalled();
  });
  it("one bar at a time: a close question replaces the pause bar, and a pause bar replaces a restart", async () => {
    const agent = { ...running, agent: "claude", mode: "plan" } as SessionInfo;
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
      context: () => ({ endpoint, infos: [agent], selectedId: "a" }),
      select: vi.fn(),
    });
    actions.pause("a");
    await actions.requestClose("a");
    expect(actions.getState().pauseBar).toBeNull();
    expect(actions.getState().closing).toBe("a");
    actions.pause("a");
    expect(actions.getState().closing).toBeNull();
  });
  it("the typing bar does not replace a question that is already up", () => {
    const agent = { ...running, agent: "claude", mode: "plan" } as SessionInfo;
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
      context: () => ({ endpoint, infos: [agent, frozen], selectedId: "a" }),
      select: vi.fn(),
    });
    actions.askRestart("a", "bypass");
    actions.showPausedBar("p");
    expect(actions.getState().pauseBar).toBeNull();
    actions.cancelRestart();
    actions.pause("a");
    actions.askRestart("a", "bypass");
    expect(actions.getState().pauseBar).toBeNull();
    expect(actions.getState().restarting).not.toBeNull();
  });
  it("pauseBarStale: the session is gone, a close question is up, focus moved, or the snapshot already agrees", () => {
    const base = { id: "a", closing: null, focusedId: "a", ids: ["a"], showing: null };
    expect(pauseBarStale({ ...base, kind: "confirm", paused: false })).toBe(false);
    expect(pauseBarStale({ ...base, kind: "confirm", paused: true })).toBe(true);
    expect(pauseBarStale({ ...base, kind: "typing", paused: true })).toBe(false);
    expect(pauseBarStale({ ...base, kind: "typing", paused: false })).toBe(true);
    expect(pauseBarStale({ ...base, kind: "typing", paused: true, closing: "a" })).toBe(true);
    expect(pauseBarStale({ ...base, kind: "typing", paused: true, ids: [] })).toBe(true);
  });
});

describe("a session running as administrator", () => {
  const admin = { ...info("a"), number: 3, name: "infra", agent: "claude", mode: "manual", elevated: true } as SessionInfo;
  const plain = { ...admin, tab_id: "p", elevated: false } as SessionInfo;
  function adminSetup(infos: SessionInfo[] = [admin, plain]) {
    const pause = vi.fn().mockResolvedValue({ ok: true });
    const restart = vi.fn().mockResolvedValue({ ok: true });
    const resume = vi.fn().mockResolvedValue({ ok: true });
    const reopen = vi.fn().mockResolvedValue({ tab_id: "n", number: 9 });
    const ask = vi.fn().mockResolvedValue({ tab_id: "n", number: 9 });
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen, rename: vi.fn(), reorder: vi.fn(), resume, restart, pause, cont: vi.fn() },
      context: () => ({ endpoint, infos, selectedId: "a", detections: [{ cli: "antigravity", installed: false, configPath: "settings.json", configState: "missing" }] }),
      select: vi.fn(),
      settings: { pauseWarned: true, markPauseWarned: vi.fn() },
      ask,
    });
    return { actions, pause, restart, resume, reopen, ask };
  }
  it("pause_is_unavailable_on_an_elevated_row: nothing is asked or sent, and the copy is the core's", () => {
    const { actions, pause } = adminSetup();
    expect(actions.pause("a")).toBeNull();
    expect(actions.getState().pauseBar).toBeNull();
    expect(pause).not.toHaveBeenCalled();
    expect(actions.pause("p")).toBe("sent");
    expect(PAUSE_UNAVAILABLE).toBe("Pause isn't available for a session running as administrator.");
  });
  it("bypass_on_an_elevated_row_needs_the_checkbox: the bar asks for it, and Restart does nothing until it is ticked", async () => {
    const { actions, restart, ask } = adminSetup();
    expect(actions.askRestart("a", "bypass")).toBe(true);
    expect(actions.restartNeedsAck()).toBe(true);
    await actions.confirmRestart(false);
    expect(ask).not.toHaveBeenCalled();
    expect(actions.getState().restarting).toEqual({ id: "a", mode: "bypass" });
    await actions.confirmRestart(true);
    // The token API refuses an administrator restart: it goes through the window.
    expect(ask).toHaveBeenCalledWith("elevated_restart", { tabId: "a", req: { mode: "bypass", cols: 120, rows: 30 } });
    expect(restart).not.toHaveBeenCalled();
  });
  it("starting_an_elevated_row_goes_through_the_window: resume and restart use the Tauri commands, a plain row the API", async () => {
    const { actions, resume, restart, ask } = adminSetup([{ ...admin, state: { kind: "ended" } } as SessionInfo, { ...plain, state: { kind: "ended" } } as SessionInfo]);
    await actions.resume("a");
    expect(ask).toHaveBeenCalledWith("elevated_resume", { tabId: "a", cols: 120, rows: 30 });
    expect(resume).not.toHaveBeenCalled();
    await actions.resume("p");
    expect(resume).toHaveBeenCalledWith(endpoint, "p", { cols: 120, rows: 30 });
    expect(ask).toHaveBeenCalledTimes(1);
    actions.askRestart("p", "plan");
    await actions.confirmRestart();
    expect(restart).toHaveBeenCalledWith(endpoint, "p", "plan", { cols: 120, rows: 30 });
    expect(ask).toHaveBeenCalledTimes(1);
  });
  it("reopen_of_a_closed_administrator_row_goes_through_the_window: the API's 409 sends the same reopen to the window", async () => {
    const { actions, reopen, ask } = adminSetup();
    reopen.mockRejectedValueOnce(new ApiError(409, "This session runs as administrator, so only Marshell's own window can start it."));
    await actions.reopenLast();
    expect(ask).toHaveBeenCalledWith("elevated_reopen", { tabId: null });
    expect(actions.getState().toast).toBeNull();
  });
  it("a switch to another mode, or any switch on a plain agent, needs no checkbox", () => {
    const { actions } = adminSetup();
    actions.askRestart("a", "plan");
    expect(actions.restartNeedsAck()).toBe(false);
    actions.askRestart("p", "bypass");
    expect(actions.restartNeedsAck()).toBe(false);
  });
  it("an_instant_switch_to_bypass_on_an_elevated_row_uses_the_bar: also when the CLI's own settings make the effective mode Bypass", () => {
    expect(needsBypassAck(admin, "bypass")).toBe(true);
    expect(needsBypassAck(admin, "plan")).toBe(false);
    expect(needsBypassAck(admin, "manual", true)).toBe(true);
    expect(needsBypassAck(admin, "plan", true)).toBe(true);
    expect(needsBypassAck(plain, "bypass")).toBe(false);
    expect(needsBypassAck({ ...admin, agent: null }, "bypass")).toBe(false);
    expect(BYPASS_ACK_TEXT).toBe("I understand this agent runs as administrator and Bypass lets it run admin commands without asking.");
  });
});

describe("resuming an administrator Antigravity session", () => {
  function resumeSetup({ failed = false, pending = false, elevated = true, wsl = false, exited = false } = {}) {
    const ask = vi.fn().mockResolvedValue({ ok: true });
    const resume = vi.fn().mockResolvedValue({ ok: true });
    const select = vi.fn();
    const ended = { ...info("a"), agent: "antigravity", elevated, mode: "plan", state: exited ? { kind: "exited", code: 0 } : { kind: "ended" } } as SessionInfo;
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume, restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
      context: () => ({ endpoint, infos: [ended], selectedId: "other", profiles: [{ id: "pwsh", family: wsl ? "wsl" : "power_shell" }], detectionFailed: new Set(failed ? ["antigravity"] : []), detections: failed || pending ? [] : [{ cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", skipsAll: { setting: "dangerouslySkipPermissions" } }] }),
      select, ask,
    });
    return { actions, ask, resume, select };
  }
  it.each([false, true])("requires the box for known skip settings (setup failed: %s)", async (failed) => {
    const { actions, ask, resume, select } = resumeSetup({ failed });
    expect(actions.resumeNeedsAck("a")).toBe(true);
    await actions.resume("a");
    expect(ask).not.toHaveBeenCalled();
    expect(resume).not.toHaveBeenCalled();
    expect(actions.getState().resuming).toEqual([]);
    expect(select).toHaveBeenCalledWith("a");
    await actions.resume("a", true);
    expect(ask).toHaveBeenCalledWith("elevated_resume", { tabId: "a", cols: 120, rows: 30 });
    expect(resume).not.toHaveBeenCalled();
  });
  it("also holds an exited row", async () => {
    const { actions, ask } = resumeSetup({ exited: true });
    await actions.resume("a");
    expect(ask).not.toHaveBeenCalled();
    await actions.resume("a", true);
    expect(ask).toHaveBeenCalledTimes(1);
  });
  it("holds an administrator Resume before settings detection arrives", async () => {
    const { actions, ask } = resumeSetup({ pending: true });
    expect(actions.resumeNeedsAck("a")).toBe(true);
    await actions.resume("a");
    expect(ask).not.toHaveBeenCalled();
  });
  it("leaves ordinary rows unchanged", async () => {
    const { actions, resume } = resumeSetup({ elevated: false, failed: true });
    expect(actions.resumeNeedsAck("a")).toBe(false);
    await actions.resume("a");
    expect(resume).toHaveBeenCalledWith(endpoint, "a", { cols: 120, rows: 30 });
  });
  it("does not apply Windows settings to a WSL row", async () => {
    const { actions, ask } = resumeSetup({ wsl: true, failed: true });
    expect(actions.resumeNeedsAck("a")).toBe(false);
    await actions.resume("a");
    expect(ask).toHaveBeenCalledTimes(1);
  });
  it("holds a restart when the settings read failed", async () => {
    const { actions, ask } = resumeSetup({ failed: true });
    actions.askRestart("a", "manual");
    expect(actions.restartNeedsAck()).toBe(true);
    await actions.confirmRestart();
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmRestart(true);
    expect(ask).toHaveBeenCalledWith("elevated_restart", { tabId: "a", req: { mode: "manual", cols: 120, rows: 30 } });
  });
});

describe("reopening a closed administrator row when Antigravity may bypass", () => {
  function reopenSetup(failed = false, pending = false, closed: SessionInfo[] = [], wsl = false) {
    const ask = vi.fn().mockResolvedValue({ tab_id: "new", number: 2 });
    const reopen = vi.fn().mockRejectedValue(new ApiError(409, "This session runs as administrator."));
    const actions = new SessionActions({
      api: { close: vi.fn(), reopen, rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
      context: () => ({ endpoint, infos: [{ ...info("a"), agent: "claude", mode: "manual" } as SessionInfo], closed, profiles: [{ id: "pwsh", family: wsl ? "wsl" : "power_shell" }], selectedId: "a", detectionFailed: new Set(failed ? ["antigravity"] : []), detections: failed || pending ? [] : [{ cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", skipsAll: { setting: "dangerouslySkipPermissions" } }] }),
      select: vi.fn(), ask,
    });
    return { actions, ask, reopen };
  }
  it.each(["claude", "codex"])("reopens a known elevated %s row without Antigravity's confirmation", async (agent) => {
    const { actions, ask } = reopenSetup(false, false, [{ ...info("closed"), agent, elevated: true } as SessionInfo]);
    await actions.reopenLast();
    expect(actions.getState().reopening).toBeNull();
    expect(ask).toHaveBeenCalledExactlyOnceWith("elevated_reopen", { tabId: null });
  });
  it.each([[false, false], [true, false], [false, true]])("asks for a known elevated Antigravity row (failed: %s, pending: %s)", async (failed, pending) => {
    const { actions, ask } = reopenSetup(failed, pending, [{ ...info("closed"), agent: "antigravity", elevated: true } as SessionInfo]);
    await actions.reopenLast();
    expect(actions.getState().reopening).toEqual({ tabId: undefined });
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmReopen(false);
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmReopen(true);
    expect(ask).toHaveBeenCalledExactlyOnceWith("elevated_reopen", { tabId: null });
  });
  it("uses the toast's closed row rather than the most recent one", async () => {
    const { actions, ask } = reopenSetup(false, false, [
      { ...info("other"), agent: "antigravity", elevated: true } as SessionInfo,
      { ...info("a"), agent: "claude", elevated: true } as SessionInfo,
    ]);
    await actions.requestClose("a");
    await actions.confirmClose();
    await actions.reopenFromToast();
    expect(actions.getState().reopening).toBeNull();
    expect(ask).toHaveBeenCalledExactlyOnceWith("elevated_reopen", { tabId: "a" });
  });
  it("keeps asking when the toast's row is missing from closed", async () => {
    const { actions, ask } = reopenSetup(false, false, [{ ...info("other"), agent: "claude", elevated: true } as SessionInfo]);
    await actions.requestClose("a");
    await actions.confirmClose();
    await actions.reopenFromToast();
    expect(actions.getState().reopening).toEqual({ tabId: "a" });
    expect(ask).not.toHaveBeenCalled();
  });
  it("does not apply Windows Antigravity settings to a WSL closed row", async () => {
    const { actions, ask } = reopenSetup(false, false, [{ ...info("closed"), agent: "antigravity", elevated: true } as SessionInfo], true);
    await actions.reopenLast();
    expect(actions.getState().reopening).toBeNull();
    expect(ask).toHaveBeenCalledExactlyOnceWith("elevated_reopen", { tabId: null });
  });
  it.each([false, true])("defers the window command until acknowledgement (failed: %s)", async (failed) => {
    const { actions, ask, reopen } = reopenSetup(failed);
    await actions.reopenLast();
    expect(reopen).toHaveBeenCalledWith(endpoint);
    expect(actions.getState().reopening).toEqual({ tabId: undefined });
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmReopen();
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmReopen(true);
    expect(ask).toHaveBeenCalledWith("elevated_reopen", { tabId: null });
    expect(actions.getState().reopening).toBeNull();
  });
  it("cancel sends nothing and restores focus", async () => {
    const { actions, ask } = reopenSetup();
    await actions.reopenLast();
    const before = actions.getState().focusToken;
    actions.cancelReopen();
    expect(actions.getState().reopening).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
    expect(ask).not.toHaveBeenCalled();
    await actions.confirmReopen(true);
    expect(ask).not.toHaveBeenCalled();
  });
  it("also defers while Antigravity's setup read is still pending", async () => {
    const { actions, ask } = reopenSetup(false, true);
    await actions.reopenLast();
    expect(actions.getState().reopening).toEqual({ tabId: undefined });
    expect(ask).not.toHaveBeenCalled();
  });
  it.each(["close", "restart", "pause"])("a %s question replaces the Reopen confirmation", async (next) => {
    const { actions, ask } = reopenSetup();
    await actions.reopenLast();
    if (next === "close") await actions.requestClose("a");
    else if (next === "restart") actions.askRestart("a", "plan");
    else actions.pause("a");
    expect(actions.getState().reopening).toBeNull();
    await actions.confirmReopen(true);
    expect(ask).not.toHaveBeenCalled();
  });
  it("an ordinary closed row still reopens through the API immediately", async () => {
    const { actions, reopen, ask } = reopenSetup();
    reopen.mockResolvedValue({ tab_id: "new", number: 2 });
    await actions.reopenLast();
    expect(actions.getState().reopening).toBeNull();
    expect(ask).not.toHaveBeenCalled();
  });
});


it("restart reads the agent detection before allowing an administrator Manual start", async () => {
  const ask = vi.fn().mockResolvedValue({ ok: true });
  const actions = new SessionActions({
    api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
    context: () => ({ endpoint, infos: [{ ...info("a"), agent: "antigravity", elevated: true, mode: "plan" } as SessionInfo], selectedId: "a", detections: [{ cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", skipsAll: { setting: "dangerouslySkipPermissions" } }] }),
    select: vi.fn(), ask,
  });
  actions.askRestart("a", "manual");
  expect(actions.restartNeedsAck()).toBe(true);
  await actions.confirmRestart();
  expect(ask).not.toHaveBeenCalled();
  await actions.confirmRestart(true);
  expect(ask).toHaveBeenCalledWith("elevated_restart", { tabId: "a", req: { mode: "manual", cols: 120, rows: 30 } });
});


it("the restart question calls a skip-every-prompt Manual launch Bypass but keeps its actual command", () => {
  expect(restartBarText({ ...info("a"), agent: "antigravity" } as SessionInfo, "manual", false, true)).toBe("Restart 1 a in Bypass? It ends and runs agy --continue.");
});


it("does not use Windows settings to request acknowledgement for a WSL Manual restart", () => {
  const actions = new SessionActions({
    api: { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(), restart: vi.fn(), pause: vi.fn(), cont: vi.fn() },
    context: () => ({ endpoint, infos: [{ ...info("a"), agent: "antigravity", elevated: true, profile_id: "wsl", mode: "plan" } as SessionInfo], selectedId: "a", profiles: [{ id: "wsl", family: "wsl" }], detections: [{ cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", skipsAll: { setting: "dangerouslySkipPermissions" } }] }),
    select: vi.fn(),
  });
  actions.askRestart("a", "manual");
  expect(actions.restartNeedsAck()).toBe(false);
});
