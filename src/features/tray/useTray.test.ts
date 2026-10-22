import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { mountHook } from "../../test/hooks";
import type { ApprovalRequest } from "../approval/types";
import type { RowModel } from "../sidebar/types";
import type { TrayState } from "./trayItems";
import { actionFor, trayClosesBoard, trayModel, traySessions, useTray, type TrayAction } from "./useTray";

vi.mock("react", async () => ({ ...await vi.importActual("react"), ...(await import("../../test/hooks")).hooks }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => vi.clearAllMocks());

describe("trayClosesBoard", () => {
  it.each([
    [{ kind: "goto", number: 2 }, "departures", true],
    [{ kind: "nextWaiting" }, "departures", true],
    [{ kind: "goto", number: 2 }, "palette", false],
    [{ kind: "nextWaiting" }, "palette", false],
    [{ kind: "goto", number: 2 }, null, false],
    [{ kind: "nextWaiting" }, null, false],
    [{ kind: "overlay", overlay: "departures" }, "departures", false],
    [{ kind: "overlay", overlay: "notifications" }, "departures", false],
    [{ kind: "togglePause" }, "departures", false],
  ] satisfies [TrayAction, string | null, boolean][])("action %j with overlay %s closes board: %s", (action, overlay, expected) => {
    expect(trayClosesBoard(action, overlay)).toBe(expected);
  });
});

const row = (over: Partial<RowModel> = {}): RowModel => ({ id: "tab-a", number: 3, name: "infra", project: "infra", projectKey: "infra", agent: "claude", status: "needs-permission", phrase: "Wants to run `npm test`", ...over });
const entry = (tab: string, over: Partial<ApprovalRequest> = {}): ApprovalRequest => ({
  id: `r-${tab}`,
  tab,
  session: { name: tab, agent: "claude", number: 9 },
  sessionCwd: "",
  waitingMs: 0,
  detail: { kind: "bash", command: "ls" },
  risk: { level: "normal" },
  state: { phase: "pending" },
  ...over,
});

describe("traySessions", () => {
  it("keeps the lane's order and takes status, number and phrase from each entry's row", () => {
    const rows = [row({ id: "a", number: 1, name: "one" }), row({ id: "b", number: 2, name: "two", status: "needs-question", phrase: "Pick a region" })];
    const s = traySessions([entry("b"), entry("a")], rows);
    expect(s.map((x) => [x.number, x.name, x.status, x.phrase])).toEqual([
      [2, "two", "needs-question", "Pick a region"],
      [1, "one", "needs-permission", "Wants to run `npm test`"],
    ]);
  });
  it("includes a terminal-only entry, which is what an expired hold leaves", () => {
    const s = traySessions([entry("a", { id: "terminal-a", terminalOnly: true })], [row({ id: "a" })]);
    expect(s).toHaveLength(1);
    expect(s[0]).toMatchObject({ number: 3, status: "needs-permission" });
  });
  it("falls back to a held request's own session when its row is missing", () => {
    const s = traySessions([entry("gone", { session: { name: "ghost", agent: "claude", number: 7 } })], []);
    expect(s).toEqual([{ number: 7, name: "ghost", status: "needs-permission", phrase: "Needs your permission", agent: "claude" }]);
  });
  it("lists a tab once, like the count does, however many requests it holds", () => {
    expect(traySessions([entry("a", { id: "1" }), entry("a", { id: "2" })], [row({ id: "a" })])).toHaveLength(1);
  });
});

describe("trayModel", () => {
  const base: TrayState = { os: "windows", sessions: [], paused: false, doctorIssues: null, departures: false };
  const wait = (n: number) => ({ number: n, name: `s${n}`, status: "needs-permission" as const, phrase: "p", agent: "claude" as const });
  it("counts every waiting session, not only the five the menu lists", () => {
    const m = trayModel({ ...base, sessions: Array.from({ length: 8 }, (_, i) => wait(i + 1)) });
    expect(m.waiting).toBe(8);
    expect(m.items.filter((i) => i.kind === "session")).toHaveLength(5);
    expect(m.tooltip).toBe("Marshell · 8 need you");
  });
  it("says in the tooltip when notifications are paused", () => {
    expect(trayModel({ ...base, paused: true }).tooltip).toBe("Marshell (notifications paused)");
    expect(trayModel(base)).toMatchObject({ waiting: 0, tooltip: "Marshell" });
  });
});

describe("actionFor", () => {
  it("goes to a session by its number", () => {
    expect(actionFor("session-7")).toEqual({ kind: "goto", number: 7 });
  });
  it("goes to the oldest waiting session for the item and for the 'more' line", () => {
    expect(actionFor("next-waiting")).toEqual({ kind: "nextWaiting" });
    expect(actionFor("more")).toEqual({ kind: "nextWaiting" });
  });
  it("opens Notifications and toggles notifications, never the palette's Pause session", () => {
    expect(actionFor("notifications")).toEqual({ kind: "overlay", overlay: "notifications" });
    expect(actionFor("pause")).toEqual({ kind: "togglePause" });
  });
  it("opens the Departures board", () => {
    expect(actionFor("departures")).toEqual({ kind: "overlay", overlay: "departures" });
  });
  it("ignores anything else", () => {
    for (const id of ["open", "quit", "doctor", "session-", "session-x", "nope"]) expect(actionFor(id)).toBeNull();
  });
});

describe("useTray", () => {
  const state = (over: Partial<TrayState> = {}): TrayState => ({ os: "windows", sessions: [], paused: false, doctorIssues: null, departures: false, ...over });
  it("sends the model once, and again only when it changes", () => {
    let s: TrayState | null = state();
    const h = mountHook(() => useTray(s));
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenLastCalledWith("set_tray", { model: expect.objectContaining({ waiting: 0, tooltip: "Marshell" }) });
    s = state();
    h.rerender();
    expect(invoke).toHaveBeenCalledTimes(1);
    s = state({ paused: true });
    h.rerender();
    expect(invoke).toHaveBeenCalledTimes(2);
    h.unmount();
  });
  it("sends nothing until the state is known, and survives a failing shell", () => {
    vi.mocked(invoke).mockRejectedValue(new Error("no tauri"));
    let s: TrayState | null = null;
    const h = mountHook(() => useTray(s));
    expect(invoke).not.toHaveBeenCalled();
    s = state();
    h.rerender();
    expect(invoke).toHaveBeenCalledTimes(1);
    h.unmount();
  });
});
