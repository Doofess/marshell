import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { Task } from "../../generated/Task";
import { FIXTURES } from "../sidebar/fixtures";
import { NOT_A_TASK, keyBlocked, markTasks, paletteHidden, reviewAgentName, reviewKeyDown, reviewTarget, taskRefresher } from "../tasks/reviewOpen";
import { ApiError } from "../../lib/api/client";
import type { RowModel } from "../sidebar/types";
import { CloseBar } from "./CloseBar";
import { SessionActions, agentLabel, closeBarText, needsCloseConfirm, tabIdOfNumber, TOAST_MS } from "./sessionActions";
import { Toast } from "./Toast";

const ep: Endpoint = { port: 1, token: "t", platform: "windows" };
const info = (n: number, patch: Partial<SessionInfo> = {}): SessionInfo => ({
  tab_id: `t${n}`,
  number: n,
  name: `Shell ${n}`,
  agent: null,
  profile_id: "pwsh",
  cwd: "C:\\dev\\my-app",
  project: "C:\\dev\\my-app",
  state: { kind: "running" },
  command: { kind: "unknown" },
  ...patch,
});
const claude = (n: number, patch: Partial<SessionInfo> = {}) => info(n, { name: "Claude Code", agent: "claude", ...patch });

function setup(infos: SessionInfo[], selectedId: string | undefined = infos[0]?.tab_id) {
  const api = {
    close: vi.fn(async () => {}),
    reopen: vi.fn(async () => ({ tab_id: "new", number: 1 })),
    rename: vi.fn(async () => {}),
    reorder: vi.fn(async () => {}),
    resume: vi.fn(async (): Promise<{ ok: true } | { ok: false; reason: string }> => ({ ok: true })),
  };
  const select = vi.fn();
  const actions = new SessionActions({ api, context: () => ({ endpoint: ep, infos, selectedId }), select });
  return { api, select, actions };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("LiveApp: close", () => {
  it("close shell asks nothing and offers reopen", async () => {
    const { api, actions } = setup([info(1), info(2)]);
    await actions.requestClose();
    expect(actions.getState().closing).toBeNull();
    expect(api.close).toHaveBeenCalledWith(ep, "t1");
    expect(actions.getState().toast).toMatchObject({ text: "Closed Shell 1.", reopenId: "t1" });
    // The toast goes after 10 seconds.
    vi.advanceTimersByTime(TOAST_MS - 1);
    expect(actions.getState().toast).not.toBeNull();
    vi.advanceTimersByTime(1);
    expect(actions.getState().toast).toBeNull();
  });

  it("closes an exited agent session at once too", async () => {
    const { api, actions } = setup([claude(1, { state: { kind: "exited", code: 0 } })]);
    await actions.requestClose();
    expect(api.close).toHaveBeenCalledWith(ep, "t1");
    expect(actions.getState().closing).toBeNull();
  });

  it("close agent shows bar with cancel focused", async () => {
    const { api, actions } = setup([claude(1)]);
    await actions.requestClose();
    expect(api.close).not.toHaveBeenCalled();
    expect(actions.getState().closing).toBe("t1");
    const text = closeBarText(claude(1));
    expect(text).toBe("Claude Code may be mid-task in my-app.");
    const h = renderToStaticMarkup(<CloseBar text={text} onCancel={() => {}} onCloseNow={() => {}} />);
    expect(h).toContain('role="alert"');
    expect(h).toContain(text);
    // Cancel is the first control and the one marked to take focus; Close now is not.
    expect(h).toMatch(/<button[^>]*data-initial-focus[^>]*>Cancel<\/button>/);
    expect(h).not.toMatch(/data-initial-focus[^>]*>Close now/);
    expect(h.indexOf("Cancel")).toBeLessThan(h.indexOf("Close now"));
  });

  it("names Codex and Antigravity by their display names", () => {
    expect(closeBarText(info(1, { agent: "codex", cwd: "/home/me/api/" }))).toBe("Codex may be mid-task in api.");
    expect(closeBarText(info(1, { agent: "antigravity" }))).toBe("Antigravity may be mid-task in my-app.");
  });

  it("names an agent for a sentence: the CLI's own name, the mark's name, or the id", () => {
    expect(agentLabel("claude")).toBe("Claude Code");
    expect(agentLabel("copilot")).toBe("GitHub Copilot");
    expect(agentLabel("my-cli")).toBe("my-cli");
  });

  it("only a running agent session asks", () => {
    expect(needsCloseConfirm(claude(1))).toBe(true);
    expect(needsCloseConfirm(claude(1, { state: { kind: "exited", code: 1 } }))).toBe(false);
    expect(needsCloseConfirm(info(1))).toBe(false);
  });

  it("Close now closes the session, and Ctrl+Shift+W again while the bar is up does not", async () => {
    const { api, actions } = setup([claude(1)]);
    await actions.requestClose();
    await actions.requestClose();
    expect(api.close).not.toHaveBeenCalled();
    await actions.confirmClose();
    expect(api.close).toHaveBeenCalledTimes(1);
    expect(actions.getState().closing).toBeNull();
    expect(actions.getState().toast?.reopenId).toBe("t1");
  });

  it("escape on close bar keeps the session", async () => {
    const { api, actions } = setup([claude(1)]);
    await actions.requestClose();
    const before = actions.getState().focusToken;
    actions.cancelClose();
    expect(actions.getState().closing).toBeNull();
    expect(api.close).not.toHaveBeenCalled();
    // Focus goes back to the terminal.
    expect(actions.getState().focusToken).toBe(before + 1);
  });

  it("tells the person when the core refuses to close", async () => {
    const { api, actions } = setup([info(1)]);
    api.close.mockRejectedValueOnce(new ApiError(500, "boom"));
    await actions.requestClose();
    expect(actions.getState().toast).toMatchObject({ text: "Couldn't close Shell 1." });
    expect(actions.getState().toast?.reopenId).toBeUndefined();
  });
});

describe("LiveApp: reopen", () => {
  it("the toast's Reopen reopens that tab and clears the toast", async () => {
    const { api, actions } = setup([info(1)]);
    await actions.requestClose();
    await actions.reopenFromToast();
    expect(api.reopen).toHaveBeenCalledWith(ep, "t1");
    expect(actions.getState().toast).toBeNull();
  });

  it("Ctrl+Shift+Alt+T reopens the most recent, with no id", async () => {
    const { api, actions } = setup([info(1)]);
    await actions.reopenLast();
    expect(api.reopen).toHaveBeenCalledTimes(1);
    expect(api.reopen.mock.calls[0]).toEqual([ep]);
    expect(actions.getState().toast).toBeNull();
  });

  it("reopen with nothing closed says so", async () => {
    const { api, actions } = setup([info(1)]);
    api.reopen.mockRejectedValueOnce(new ApiError(404, "nothing to reopen"));
    await actions.reopenLast();
    expect(actions.getState().toast).toMatchObject({ text: "Nothing to reopen." });
    expect(actions.getState().toast?.reopenId).toBeUndefined();
  });

  it("says why when reopening fails for another reason", async () => {
    const { api, actions } = setup([info(1)]);
    api.reopen.mockRejectedValueOnce(new ApiError(500, "no such folder"));
    await actions.reopenLast();
    expect(actions.getState().toast?.text).toBe("Couldn't reopen the session: no such folder");
  });
});

describe("LiveApp: go to session", () => {
  it("ctrl+shift+3 selects number 3 not third row", () => {
    // The rows are reordered and 2 is gone: number 3 is the second row, and the third row is number 1.
    const infos = [info(4), info(3), info(1)];
    expect(tabIdOfNumber(infos, 3)).toBe("t3");
    const { select, actions } = setup(infos);
    actions.handle({ kind: "goto", number: 3 });
    expect(select).toHaveBeenCalledWith("t3");
  });

  it("does nothing when no session has that number", () => {
    const { select, actions } = setup([info(1), info(3)]);
    actions.handle({ kind: "goto", number: 2 });
    expect(select).not.toHaveBeenCalled();
  });
});

describe("LiveApp: rename and reorder", () => {
  it("F2 starts renaming the selected session", () => {
    const { actions } = setup([info(1), info(2)], "t2");
    actions.handle({ kind: "rename" });
    expect(actions.getState().renaming).toBe("t2");
  });

  it("F2 with nothing selected does nothing", () => {
    const { actions } = setup([], undefined);
    actions.handle({ kind: "rename" });
    expect(actions.getState().renaming).toBeNull();
  });

  it("a committed name goes to the core and ends the edit", async () => {
    const { api, actions } = setup([info(1)]);
    actions.startRename();
    await actions.commitRename("t1", "api work");
    expect(api.rename).toHaveBeenCalledWith(ep, "t1", "api work");
    expect(actions.getState().renaming).toBeNull();
  });

  it("cancelling a rename sends nothing; blur does not take focus back from where the person clicked", () => {
    const { api, actions } = setup([info(1)]);
    actions.startRename();
    const before = actions.getState().focusToken;
    actions.cancelRename({ refocus: false });
    expect(actions.getState().renaming).toBeNull();
    expect(actions.getState().focusToken).toBe(before);
    actions.startRename();
    actions.cancelRename();
    expect(actions.getState().focusToken).toBe(before + 1);
    expect(api.rename).not.toHaveBeenCalled();
  });

  it("says so when the core refuses a name", async () => {
    const { api, actions } = setup([info(1)]);
    api.rename.mockRejectedValueOnce(new ApiError(400, "name too long"));
    await actions.commitRename("t1", "x");
    expect(actions.getState().toast?.text).toBe("Couldn't rename the session: name too long");
  });

  it("Alt+Shift+arrow sends the new order and changes nothing on screen by itself", async () => {
    const { api, actions } = setup([info(1), info(2), info(3)], "t2");
    actions.handle({ kind: "move", by: -1 });
    expect(api.reorder).toHaveBeenCalledWith(ep, ["t2", "t1", "t3"]);
  });

  it("while grouped, Alt+Shift+arrow moves a session only inside its project and sends the displayed order", () => {
    const infos = [info(1), info(2), info(3), info(4)];
    const groupedRows = [{ id: "t1", project: "a", projectKey: "a" }, { id: "t2", project: "b", projectKey: "b" }, { id: "t3", project: "a", projectKey: "a" }, { id: "t4", project: "b", projectKey: "b" }] as RowModel[];
    const api = { close: vi.fn(async () => {}), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(async () => {}), resume: vi.fn() };
    let selectedId = "t3";
    const actions = new SessionActions({ api, context: () => ({ endpoint: ep, infos, selectedId, groupedRows }), select: vi.fn() });
    actions.handle({ kind: "move", by: -1 });
    expect(api.reorder).toHaveBeenCalledWith(ep, ["t3", "t1", "t2", "t4"]);
    // The group edge: t3 is last of a (displayed t1, t3, t2, t4), so down goes nowhere, and t2 cannot move up into a.
    api.reorder.mockClear();
    actions.handle({ kind: "move", by: 1 });
    selectedId = "t2";
    actions.handle({ kind: "move", by: -1 });
    expect(api.reorder).not.toHaveBeenCalled();
  });

  it("does not send a move past the end of the list", () => {
    const { api, actions } = setup([info(1), info(2)], "t1");
    actions.handle({ kind: "move", by: -1 });
    expect(api.reorder).not.toHaveBeenCalled();
  });

  it("a drag sends the dropped order", () => {
    const { api, actions } = setup([info(1), info(2)]);
    actions.reorder(["t2", "t1"]);
    expect(api.reorder).toHaveBeenCalledWith(ep, ["t2", "t1"]);
  });
});

describe("LiveApp: fix round 1", () => {
  const deferred = () => {
    let resolve!: () => void;
    const promise = new Promise<void>((res) => (resolve = res));
    return { promise, resolve };
  };

  it("Close now hands focus back to the terminal", async () => {
    const { actions } = setup([claude(1)]);
    await actions.requestClose();
    const before = actions.getState().focusToken;
    await actions.confirmClose();
    expect(actions.getState().closing).toBeNull();
    expect(actions.getState().focusToken).toBeGreaterThan(before);
  });

  it("Close now hands focus back even when the close fails", async () => {
    const { api, actions } = setup([claude(1)]);
    api.close.mockRejectedValueOnce(new Error("gone"));
    await actions.requestClose();
    const before = actions.getState().focusToken;
    await actions.confirmClose();
    expect(actions.getState().toast?.text).toBe("Couldn't close Claude Code.");
    expect(actions.getState().focusToken).toBeGreaterThan(before);
  });

  it("a second close press while the first is in flight does nothing", async () => {
    const { api, actions } = setup([info(1), info(2)]);
    const slow = deferred();
    api.close.mockReturnValueOnce(slow.promise);
    const first = actions.requestClose();
    await actions.requestClose();
    expect(api.close).toHaveBeenCalledTimes(1);
    slow.resolve();
    await first;
    expect(actions.getState().toast).toMatchObject({ text: "Closed Shell 1." });
  });

  it("Close now is not sent twice for a tab already being closed", async () => {
    const { api, actions } = setup([claude(1)]);
    await actions.requestClose();
    const slow = deferred();
    api.close.mockReturnValueOnce(slow.promise);
    const first = actions.confirmClose();
    await actions.requestClose();
    expect(api.close).toHaveBeenCalledTimes(1);
    slow.resolve();
    await first;
  });

  it("a tab can be closed again after a failed close", async () => {
    const { api, actions } = setup([info(1)]);
    api.close.mockRejectedValueOnce(new Error("busy"));
    await actions.requestClose();
    await actions.requestClose();
    expect(api.close).toHaveBeenCalledTimes(2);
  });

  it("F2 does nothing when the sidebar is not expanded, so no rename box is left pending", () => {
    const infos = [info(1)];
    const api = { close: vi.fn(async () => {}), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn() };
    const actions = new SessionActions({ api, context: () => ({ endpoint: ep, infos, selectedId: "t1", sidebarExpanded: false }), select: vi.fn() });
    actions.startRename();
    expect(actions.getState().renaming).toBeNull();
  });
});

describe("LiveApp: resume", () => {
  const ended = (n: number, patch: Partial<SessionInfo> = {}) => claude(n, { state: { kind: "ended" }, ...patch });
  const deferred = <T,>() => {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((res) => (resolve = res));
    return { promise, resolve };
  };

  it("sends the last measured size and says Resumed", async () => {
    const infos = [ended(1)];
    const api = { close: vi.fn(), reopen: vi.fn(), rename: vi.fn(), reorder: vi.fn(), resume: vi.fn(async (): Promise<{ ok: true }> => ({ ok: true })) };
    const actions = new SessionActions({ api, context: () => ({ endpoint: ep, infos, selectedId: "t1" }), select: vi.fn(), size: () => ({ cols: 90, rows: 24 }) });
    await actions.resume("t1");
    expect(api.resume).toHaveBeenCalledWith(ep, "t1", { cols: 90, rows: 24 });
    expect(actions.getState().toast).toMatchObject({ text: "Resumed" });
    expect(actions.getState().toast?.reopenId).toBeUndefined();
  });

  it("a refused resume announces the reason", async () => {
    const { api, actions } = setup([ended(1)]);
    api.resume.mockResolvedValueOnce({ ok: false, reason: "Claude Code isn't installed or isn't on PATH." });
    await actions.resume("t1");
    // The window's announcer says the toast's text; the reason is the core's own words.
    expect(actions.getState().toast).toMatchObject({ text: "Claude Code isn't installed or isn't on PATH." });
    expect(actions.getState().resuming).toEqual([]);
    expect(actions.getState().resumed).toEqual({});
  });

  it("a failed request says so, and the tab can be resumed again", async () => {
    const { api, actions } = setup([ended(1)]);
    api.resume.mockRejectedValueOnce(new ApiError(500, "boom"));
    await actions.resume("t1");
    expect(actions.getState().toast?.text).toMatch(/^Couldn't resume the session/);
    await actions.resume("t1");
    expect(api.resume).toHaveBeenCalledTimes(2);
  });

  it("is sent once for a tab already being resumed, and a second press shows no error", async () => {
    const { api, actions } = setup([ended(1)]);
    const slow = deferred<{ ok: true }>();
    api.resume.mockReturnValueOnce(slow.promise);
    const first = actions.resume("t1");
    await actions.resume("t1");
    expect(api.resume).toHaveBeenCalledTimes(1);
    expect(actions.getState().toast).toBeNull();
    slow.resolve({ ok: true });
    await first;
    expect(actions.getState().toast).toMatchObject({ text: "Resumed" });
  });

  it("lets the terminal attach only once the response is back: the core announces Running before the session exists", async () => {
    const { api, actions } = setup([ended(1)]);
    const slow = deferred<{ ok: true }>();
    api.resume.mockReturnValueOnce(slow.promise);
    const pending = actions.resume("t1");
    // While the request is out, the tab is held back and its terminal key has not moved.
    expect(actions.getState().resuming).toEqual(["t1"]);
    expect(actions.getState().resumed.t1 ?? 0).toBe(0);
    slow.resolve({ ok: true });
    await pending;
    // One state change both frees the tab and moves its key, so the terminal never mounts in between.
    expect(actions.getState().resuming).toEqual([]);
    expect(actions.getState().resumed.t1).toBe(1);
  });

  it("does nothing for a session that is running or gone", async () => {
    const { api, actions } = setup([claude(1)]);
    await actions.resume("t1");
    await actions.resume("nope");
    expect(api.resume).not.toHaveBeenCalled();
  });
});

describe("Toast", () => {
  it("shows the text and a Reopen button, and is not a live region (the window's announcer says it)", () => {
    const h = renderToStaticMarkup(<Toast text="Closed Shell 1." actionLabel="Reopen" onAction={() => {}} />);
    expect(h).toContain("Closed Shell 1.");
    expect(h).toContain(">Reopen</button>");
    expect(h).not.toMatch(/role="(status|alert)"|aria-live/);
  });
  it("has no button when there is nothing to do", () => {
    expect(renderToStaticMarkup(<Toast text="Nothing to reopen." />)).not.toContain("<button");
  });
});

const TASK: Task = { id: "k1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:\\dev\\my-app", path: "C:\\dev\\my-app-task", created_ms: 1 };
const inTask = (n: number, patch: Partial<SessionInfo> = {}) => claude(n, { cwd: "C:\\dev\\my-app-task", ...patch });

/** A key event that records preventDefault, like a real one's defaultPrevented. */
function keydown(init: { code: string; ctrlKey?: boolean; shiftKey?: boolean; metaKey?: boolean; repeat?: boolean }) {
  const e = {
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    repeat: false,
    ...init,
    defaultPrevented: false,
    preventDefault() {
      e.defaultPrevented = true;
    },
  };
  return e;
}
const CTRL_SHIFT_R = { code: "KeyR", ctrlKey: true, shiftKey: true };

describe("LiveApp: Review this task's changes (Ctrl+Shift+R)", () => {
  const ctx = (infos: SessionInfo[], focusedId: string | undefined, tasks: Task[], blocked = false) => ({ infos, focusedId, tasks, blocked });

  it("opens the sheet for the active session in a task, and prevents the webview's hard reload", () => {
    const e = keydown(CTRL_SHIFT_R);
    const result = reviewKeyDown(e, "windows", ctx([info(1), inTask(2)], "t2", [TASK]));
    expect(result).toEqual({ kind: "open", tabId: "t2", task: TASK });
    expect(e.defaultPrevented).toBe(true);
  });

  it("does nothing for a session that is not a task, says so, and still prevents the hard reload", () => {
    const e = keydown(CTRL_SHIFT_R);
    const result = reviewKeyDown(e, "windows", ctx([info(1), inTask(2)], "t1", [TASK]));
    expect(result).toEqual({ kind: "none", message: "This session isn't a task." });
    expect(NOT_A_TASK).toBe("This session isn't a task.");
    expect(e.defaultPrevented).toBe(true);
  });

  it("says the same with no session at all", () => {
    expect(reviewTarget([], undefined, [TASK])).toEqual({ kind: "none", message: NOT_A_TASK });
    expect(reviewTarget([info(1)], "gone", [TASK])).toEqual({ kind: "none", message: NOT_A_TASK });
  });

  it("owns the key on every system: Cmd+R on macOS is prevented too", () => {
    const e = keydown({ code: "KeyR", metaKey: true });
    expect(reviewKeyDown(e, "mac", ctx([inTask(1)], "t1", [TASK]))).toMatchObject({ kind: "open" });
    expect(e.defaultPrevented).toBe(true);
  });

  it("is prevented but ignored while something else holds the keys (the launcher, the palette, the sheet, a text box) and on a held key", () => {
    const e = keydown(CTRL_SHIFT_R);
    expect(reviewKeyDown(e, "windows", ctx([inTask(1)], "t1", [TASK], true))).toEqual({ kind: "ignored" });
    expect(e.defaultPrevented).toBe(true);
    const held = keydown({ ...CTRL_SHIFT_R, repeat: true });
    expect(reviewKeyDown(held, "windows", ctx([inTask(1)], "t1", [TASK]))).toEqual({ kind: "ignored" });
    expect(held.defaultPrevented).toBe(true);
  });

  it("leaves every other key alone", () => {
    for (const init of [{ code: "KeyR" }, { code: "KeyR", ctrlKey: true }, { code: "KeyK", ctrlKey: true, shiftKey: true }]) {
      const e = keydown(init);
      expect(reviewKeyDown(e, "windows", ctx([inTask(1)], "t1", [TASK]))).toBeNull();
      expect(e.defaultPrevented).toBe(false);
    }
  });

  it("is blocked by an open launcher, palette, profiles page or review sheet, and by a text box", () => {
    const none = { launcherOpen: false, overlay: null, review: false, inTextBox: false };
    expect(keyBlocked(none)).toBe(false);
    expect(keyBlocked({ ...none, launcherOpen: true })).toBe(true);
    expect(keyBlocked({ ...none, overlay: "palette" })).toBe(true);
    expect(keyBlocked({ ...none, review: true })).toBe(true);
    expect(keyBlocked({ ...none, inTextBox: true })).toBe(true);
  });
});

describe("LiveApp: which sessions are tasks", () => {
  it("marks the rows of sessions that run in a task's folder, and only those", () => {
    const rows = [
      { ...FIXTURES[0]!, id: "t1" },
      { ...FIXTURES[1]!, id: "t2" },
    ];
    const marked = markTasks(rows, [info(1), inTask(2)], [TASK]);
    expect(marked.map((r) => r.task)).toEqual([undefined, true]);
    // A row that is not a task is the very same object, so the sidebar does not redraw it.
    expect(marked[0]).toBe(rows[0]);
    expect(markTasks(rows, [info(1), inTask(2)], [])).toBe(rows);
  });
  it("names the agent only when the task's session is a running agent", () => {
    expect(reviewAgentName(inTask(1))).toBe("Claude Code");
    expect(reviewAgentName(inTask(1, { state: { kind: "exited", code: 0 } }))).toBeNull();
    expect(reviewAgentName(info(1, { cwd: "C:\\dev\\my-app-task" }))).toBeNull();
    expect(reviewAgentName(undefined)).toBeNull();
  });
});

describe("LiveApp: the palette's Review command", () => {
  it("is hidden unless the active session is a task, and Resume is hidden unless it can resume", () => {
    expect(paletteHidden({ resumable: false, isTask: false })).toEqual(["resume", "review-task"]);
    expect(paletteHidden({ resumable: false, isTask: true })).toEqual(["resume"]);
    expect(paletteHidden({ resumable: true, isTask: false })).toEqual(["review-task"]);
    expect(paletteHidden({ resumable: true, isTask: true })).toEqual([]);
  });
});

describe("LiveApp: the task list", () => {
  const other: Task = { ...TASK, id: "k2", branch: "marshell/other" };
  it("asks the core every time it is refreshed (on start, and after every create, merge or remove)", async () => {
    const list = vi.fn(async () => [TASK]);
    const seen: Task[][] = [];
    const refresh = taskRefresher(list, (t) => seen.push(t));
    await refresh();
    await refresh();
    await refresh();
    expect(list).toHaveBeenCalledTimes(3);
    expect(seen).toHaveLength(3);
  });
  it("shows the newest answer: an older, slower one that arrives later is dropped", async () => {
    let slow!: (t: Task[]) => void;
    const answers = [new Promise<Task[]>((r) => (slow = r)), Promise.resolve([other])];
    const seen: Task[][] = [];
    const refresh = taskRefresher(() => answers.shift()!, (t) => seen.push(t));
    const first = refresh();
    await refresh();
    slow([TASK]);
    await first;
    expect(seen).toEqual([[other]]);
  });
  it("keeps the list it has when the core does not answer", async () => {
    const seen: Task[][] = [];
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    await taskRefresher(async () => Promise.reject(new Error("down")), (t) => seen.push(t))();
    expect(seen).toEqual([]);
    warn.mockRestore();
  });
});

describe("LiveApp: the sheet hands the focus to the agent's terminal", () => {
  it("notify shows the receipt as a toast with no action", () => {
    const { actions } = setup([claude(1)]);
    actions.notify("Merged marshell/fix-login into main.");
    expect(actions.getState().toast).toMatchObject({ text: "Merged marshell/fix-login into main." });
    expect(actions.getState().toast?.reopenId).toBeUndefined();
    actions.dispose();
  });
  it("focusTerminal bumps the focus token once", () => {
    const { actions } = setup([claude(1)]);
    const before = actions.getState().focusToken;
    actions.focusTerminal();
    expect(actions.getState().focusToken).toBe(before + 1);
  });
});
