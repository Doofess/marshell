import { describe, expect, it } from "vitest";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import type { LiveStatus } from "../../generated/LiveStatus";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";
import { holdsResume, offersModeChange, offersResume, resumeDisabledReason, sessionRunning } from "./resumeState";
import { toRow } from "./toRow";

const profile = (id: string) => ({ id }) as Profile;
const info = (patch: Partial<SessionInfo> = {}): SessionInfo => ({
  tab_id: "t1",
  number: 1,
  name: "Claude Code",
  agent: "claude",
  profile_id: "pwsh",
  cwd: "C:\\dev\\my-app",
  project: "C:\\dev\\my-app",
  mode: null,
  state: { kind: "running" },
  command: { kind: "unknown" },
  paused: false,
  foreground: null,
  live: null,
  activity: null,
  status_note: null,
  muted: false,
  attention: null,
  stuck: null,
  ...patch,
});

describe("offersResume", () => {
  it.each<SessionInfo["state"]>([{ kind: "exited", code: 0 }, { kind: "ended" }])("offers Resume when the process is $kind", (state) => {
    const session = info({ state });
    expect(offersResume(toRow(session, []), session)).toBe(true);
  });
  it("hides Resume when the agent ended but its shell is still running", () => {
    const session = info({ live: { state: "ended", phrase: "Ended", since_ms: 0, recap: null } as LiveStatus });
    const row = toRow(session, []);
    expect(row.status).toBe("ended");
    expect(offersResume(row, session)).toBe(false);
    expect(offersResume(row, { ...session, state: { kind: "exited", code: 0 } })).toBe(true);
  });
  it("keeps Resume for an ended row whose info is unavailable", () => {
    expect(offersResume(toRow(info({ state: { kind: "ended" } }), []), undefined)).toBe(true);
  });
  it.each((Object.keys(GLYPHS) as RowModel["status"][]).filter((status) => status !== "ended"))("never offers Resume for status %s, whatever the process state", (status) => {
    const row = { ...toRow(info(), []), status };
    for (const session of [info(), info({ state: { kind: "exited", code: 0 } }), info({ state: { kind: "ended" } }), undefined]) {
      expect(offersResume(row, session)).toBe(false);
    }
  });
});

describe("offersModeChange", () => {
  it("hides Mode only while the ended agent's shell still runs", () => {
    const session = info({ live: { state: "ended", phrase: "Ended", since_ms: 0, recap: null } as LiveStatus });
    const row = toRow(session, []);
    expect(row.status).toBe("ended");
    expect(offersModeChange(row, session)).toBe(false);
    for (const state of [{ kind: "exited", code: 0 }, { kind: "ended" }] as const) {
      expect(offersModeChange(row, { ...session, state })).toBe(true);
    }
    expect(offersModeChange(row, undefined)).toBe(true);
  });
  it.each((Object.keys(GLYPHS) as RowModel["status"][]).filter((status) => status !== "ended"))("keeps Mode for status %s, whatever the process state", (status) => {
    const row = { ...toRow(info(), []), status };
    for (const session of [info(), info({ state: { kind: "exited", code: 0 } }), info({ state: { kind: "ended" } }), undefined]) {
      expect(offersModeChange(row, session)).toBe(true);
    }
  });
});

describe("resumeDisabledReason", () => {
  it("names a profile that is missing from a loaded list", () => {
    expect(resumeDisabledReason("gone", [profile("pwsh")])).toBe("Its shell profile was removed.");
  });
  it("is silent for a profile that is in the list", () => {
    expect(resumeDisabledReason("pwsh", [profile("pwsh")])).toBeUndefined();
  });
  it("treats an empty list as unknown: a failed fetch is not a removed profile", () => {
    expect(resumeDisabledReason("pwsh", [])).toBeUndefined();
  });
});

describe("holdsResume", () => {
  const pressed = "Ended last time";
  it("holds while the request is out, whatever the live state says", () => {
    expect(holdsResume(pressed, pressed, "pending")).toBe(true);
    expect(holdsResume(pressed, "live", "pending")).toBe(true);
  });
  it("the ok response before the Running frame keeps holding, so a second press finds the strip", () => {
    expect(holdsResume(pressed, pressed, "ok")).toBe(true);
  });
  it("the ok response after the Running frame lets the live row decide", () => {
    expect(holdsResume(pressed, "live", "ok")).toBe(false);
  });
  it("a refusal drops the hold at once, and a rollback frame after it changes nothing", () => {
    expect(holdsResume(pressed, "live", "refused")).toBe(false);
    expect(holdsResume(pressed, pressed, "refused")).toBe(false);
  });
});

describe("sessionRunning", () => {
  const live = (state: LiveStatus["state"]) => ({ state, phrase: "", since_ms: 0, recap: null }) as LiveStatus;
  it("follows the process, except for an agent that has ended", () => {
    expect(sessionRunning(info())).toBe(true);
    expect(sessionRunning(info({ live: live("idle") }))).toBe(true);
    expect(sessionRunning(info({ live: live("ended") }))).toBe(false);
    expect(sessionRunning(info({ agent: null, live: live("ended") }))).toBe(true);
    expect(sessionRunning(info({ state: { kind: "exited", code: 0 } }))).toBe(false);
    expect(sessionRunning(info({ state: { kind: "ended" } }))).toBe(false);
  });
});
