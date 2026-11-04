import { describe, expect, it } from "vitest";
import { LAUNCH_PROGRAMS } from "../launcher/launchChoice";
import type { Detection } from "../onboarding/types";
import { launcherSetup, shownNumber } from "./launcherSetup";

const detected = (skipsAll = false): Detection => ({ cli: "antigravity", installed: true, configPath: "settings.json", configState: "ok", ...(skipsAll ? { skipsAll: { setting: "dangerouslySkipPermissions" } } : {}) });
const programs = [...LAUNCH_PROGRAMS, { ...LAUNCH_PROGRAMS[2]!, id: "preset:agy", agent: "antigravity" }];

describe("launcher setup read state", () => {
  it("carries an administrator refusal to the agent and tool presets that use it", () => {
    const reason = "Marshell couldn't read this Codex's version, so it can't start it as administrator.";
    const codex: Detection = { cli: "codex", installed: true, configPath: "config.toml", configState: "ok", adminUnavailable: reason };
    const choices = [...programs, { ...LAUNCH_PROGRAMS[1]!, id: "preset:codex", agent: "codex", group: "Tools" as const }];
    const result = launcherSetup(choices, false, [codex]);
    expect(result.filter((p) => p.adminUnavailable).map((p) => p.id)).toEqual(["codex", "preset:codex"]);
    expect(result.find((p) => p.id === "codex")?.adminUnavailable).toBe(reason);
    expect(launcherSetup(choices, true, [codex]).filter((p) => p.adminUnavailable).map((p) => p.id)).toEqual(["codex", "preset:codex"]);
  });
  it("only holds Antigravity and its presets while settings are pending", () => {
    const result = launcherSetup(programs, false);
    expect(result.filter((p) => p.adminSetupPending).map((p) => p.id)).toEqual(["antigravity", "preset:agy"]);
    expect(result.find((p) => p.id === "claude")?.adminSetupPending).toBe(false);
    expect(result.find((p) => p.id === "codex")?.adminSetupPending).toBe(false);
  });
  it("ends the hold for a successful read, including a setting that is off", () => {
    for (const on of [true, false]) {
      const result = launcherSetup(programs, false, [detected(on)]);
      expect(result.some((p) => p.adminSetupPending)).toBe(false);
      expect(result.find((p) => p.id === "preset:agy")?.skipsAll !== undefined).toBe(on);
    }
  });
  it("ends a failed read's hold and records uncertainty for Antigravity and its presets", () => {
    const result = launcherSetup(programs, false, [], new Set(["antigravity", "claude", "codex"]));
    expect(result.some((p) => p.adminSetupPending)).toBe(false);
    expect(result.filter((p) => p.adminSetupFailed).map((p) => p.id)).toEqual(["antigravity", "preset:agy"]);
  });
  it("treats a failed refresh as unknown even when an old detection said off", () => {
    expect(launcherSetup(programs, false, [detected()], new Set(["antigravity"]))[2]?.adminSetupFailed).toBe(true);
  });
  it("does not apply Windows settings or failures to WSL", () => {
    const result = launcherSetup(programs, true, [detected(true)], new Set(["antigravity"]));
    expect(result.some((p) => p.adminSetupPending || p.adminSetupFailed || p.skipsAll)).toBe(false);
  });
});

describe("shownNumber", () => {
  it("keeps the number a pending start reserved, and follows the live one otherwise", () => {
    expect(shownNumber(3, false, 2)).toBe(3);
    expect(shownNumber(3, true, 2)).toBe(2);
    expect(shownNumber(3, true, undefined)).toBe(3);
  });
});
