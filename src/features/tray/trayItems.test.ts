import { describe, expect, it } from "vitest";
import { badgeFor, MAX_TRAY_SESSIONS, trayItems, trayTooltip, type TrayState } from "./trayItems";

const wait = (n: number, name: string, status: "needs-permission" | "needs-question" = "needs-permission") => ({ number: n, name, status, phrase: status === "needs-permission" ? "Wants to run `npm test`" : "Has a question", agent: "claude" as const });
const state: TrayState = { os: "windows", sessions: [wait(3, "api-server"), wait(2, "infra", "needs-question")], paused: false, doctorIssues: 4, departures: true };
const ids = (s: TrayState) => trayItems(s).map((i) => i.id);

describe("trayItems", () => {
  it("opens the window first, lists who needs you with their numbers, then the actions, then quits last", () => {
    const items = trayItems(state);
    expect(items[0]).toMatchObject({ id: "open", label: "Open Marshell" });
    expect(items.filter((i) => i.kind === "session").map((i) => `${i.number} · ${i.label}`)).toEqual(["3 · api-server: Wants to run `npm test`", "2 · infra: Has a question"]);
    expect(items[items.length - 1]).toMatchObject({ id: "quit", label: "Quit Marshell" });
  });
  it("says nothing needs you when that is so, as a disabled line", () => {
    const items = trayItems({ ...state, sessions: [] });
    expect(items.find((i) => i.id === "none")).toMatchObject({ label: "Nothing needs you", disabled: true });
  });
  it("shows at most five, then how many more", () => {
    const many = Array.from({ length: 8 }, (_, i) => wait(i + 1, `s${i}`));
    const items = trayItems({ ...state, sessions: many });
    expect(items.filter((i) => i.kind === "session")).toHaveLength(MAX_TRAY_SESSIONS);
    expect(items.find((i) => i.id === "more")!.label).toBe("3 more…");
  });
  it("offers to go to the oldest waiting session with the real shortcut for the OS", () => {
    expect(trayItems(state).find((i) => i.id === "next-waiting")!.shortcut).toBe("Ctrl+Shift+N");
    expect(trayItems({ ...state, os: "mac" }).find((i) => i.id === "next-waiting")!.shortcut).toBe("⌘N");
  });
  it("toggles pause and resume, and names the doctor's count", () => {
    expect(trayItems(state).find((i) => i.id === "pause")!.label).toBe("Pause notifications");
    expect(trayItems({ ...state, paused: true }).find((i) => i.id === "pause")!.label).toBe("Resume notifications");
    expect(trayItems(state).find((i) => i.id === "doctor")!.label).toBe("Doctor: 4 issues");
    expect(trayItems({ ...state, doctorIssues: 0 }).find((i) => i.id === "doctor")!.label).toBe("Doctor: all clear");
    expect(trayItems({ ...state, doctorIssues: 1 }).find((i) => i.id === "doctor")!.label).toBe("Doctor: 1 issue");
  });
  it("offers Open Departures right after Open Marshell, named for the board", () => {
    const items = trayItems(state);
    expect(items[1]).toMatchObject({ id: "departures", label: "Open Departures", kind: "item" });
  });
  it("leaves out Departures and the Doctor in the live app, which has neither to open", () => {
    const live = { ...state, doctorIssues: null, departures: false };
    expect(ids({ ...live, sessions: [] })).toEqual(["open", "sep-1", "none", "next-waiting", "sep-2", "pause", "notifications", "sep-3", "quit"]);
    expect(ids(live)).toEqual(["open", "sep-1", "session-3", "session-2", "next-waiting", "sep-2", "pause", "notifications", "sep-3", "quit"]);
  });
  it("opens Notifications where Settings used to be, and the design state still lists Departures and the Doctor", () => {
    expect(trayItems(state).find((i) => i.id === "notifications")).toMatchObject({ label: "Notifications…", kind: "item" });
    expect(ids(state)).not.toContain("settings");
    expect(ids(state)).toEqual(expect.arrayContaining(["departures", "doctor"]));
  });
  it("never lists a session that is not waiting", () => {
    const s = { ...state, sessions: [{ ...wait(1, "busy"), status: "working" as const }] };
    expect(ids(s)).not.toContain("session-1");
  });
});

describe("trayTooltip", () => {
  it("says how many need you, and when notifications are paused", () => {
    expect(trayTooltip(state)).toBe("Marshell · 2 need you");
    expect(trayTooltip({ ...state, sessions: [wait(1, "a")] })).toBe("Marshell · 1 needs you");
    expect(trayTooltip({ ...state, sessions: [] })).toBe("Marshell");
    expect(trayTooltip({ ...state, paused: true })).toBe("Marshell · 2 need you (notifications paused)");
  });
});

describe("badgeFor", () => {
  it("uses an overlay icon on Windows (the badge count is not supported there), capped at 9+", () => {
    expect(badgeFor("windows", 2)).toEqual({ kind: "overlay", text: "2" });
    expect(badgeFor("windows", 12)).toEqual({ kind: "overlay", text: "9+" });
  });
  it("uses the dock badge on macOS and nothing on Linux, where the tray is menu-only", () => {
    expect(badgeFor("mac", 3)).toEqual({ kind: "dock", text: "3" });
    expect(badgeFor("linux", 3)).toEqual({ kind: "none", text: "" });
  });
  it("shows nothing for zero", () => {
    for (const os of ["windows", "mac", "linux"] as const) expect(badgeFor(os, 0).text).toBe("");
  });
});
