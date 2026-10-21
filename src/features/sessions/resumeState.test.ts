import { describe, expect, it } from "vitest";
import { GLYPHS } from "../../components/StatusGlyph/glyphs";
import type { LiveStatus } from "../../generated/LiveStatus";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { RowModel } from "../sidebar/types";
import { holdsResume, offersResume, resumeDisabledReason } from "./resumeState";
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
