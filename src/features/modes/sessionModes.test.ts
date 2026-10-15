import { describe, expect, it } from "vitest";
import { commandById } from "../../lib/keymap";
import { decide, effectiveMode, launchArgs, MODES, switchKind, type Request } from "./sessionModes";

const edit: Request = { kind: "edit", inProject: true };
const editOutside: Request = { kind: "edit", inProject: false };
const command: Request = { kind: "command", inProject: true };

describe("the five modes", () => {
  it("are the same five for every agent CLI, strictest first", () => {
    expect(MODES).toEqual(["manual", "plan", "auto-edit", "full-auto", "bypass"]);
  });
  it("each has a palette command that names the mode and says in plain words what asks", () => {
    for (const mode of MODES) {
      const c = commandById(`mode-${mode}`);
      expect(c.group).toBe("Sessions");
      expect(c.label).toMatch(/^Mode: (Manual|Plan|Auto-edit|Full auto|Bypass), /);
    }
    expect(commandById("mode-bypass").label).toContain("risky");
  });
});

describe("launchArgs: with Marshell's hooks on, a session starts strict and the hook relaxes it", () => {
  it("Manual, Auto-edit and Bypass start each CLI in its strictest native mode, so every request reaches Marshell", () => {
    for (const mode of ["manual", "auto-edit", "bypass"] as const) {
      expect(launchArgs("claude", mode, true), mode).toEqual({ args: ["--permission-mode", "manual"], native: mode === "manual" });
      expect(launchArgs("codex", mode, true), mode).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: mode === "manual" });
      expect(launchArgs("antigravity", mode, true), mode).toEqual({ args: [], native: mode === "manual" });
    }
  });
  it("Plan uses the CLI's own plan mode; Codex has no plan flag, so a read-only sandbox stands in", () => {
    expect(launchArgs("claude", "plan", true)).toEqual({ args: ["--permission-mode", "plan"], native: true });
    expect(launchArgs("antigravity", "plan", true)).toEqual({ args: ["--mode", "plan"], native: true });
    expect(launchArgs("codex", "plan", true)).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: false });
    expect(launchArgs("codex", "plan", false)).toEqual({ args: ["--sandbox", "read-only", "--ask-for-approval", "on-request"], native: false });
  });
  it("Full auto uses the CLI's own classifier where it has one; Antigravity has none, so Marshell's risk check stands in", () => {
    expect(launchArgs("claude", "full-auto", true)).toEqual({ args: ["--permission-mode", "auto"], native: true });
    expect(launchArgs("codex", "full-auto", true)).toEqual({ args: ["--approve-for-me"], native: true });
    expect(launchArgs("antigravity", "full-auto", true)).toEqual({ args: [], native: false });
  });
});

describe("launchArgs: with the hooks off, each mode is the CLI's own flag, or not offered", () => {
  it("Claude", () => {
    expect(launchArgs("claude", "auto-edit", false)).toEqual({ args: ["--permission-mode", "acceptEdits"], native: true });
    expect(launchArgs("claude", "bypass", false)).toEqual({ args: ["--permission-mode", "bypassPermissions"], native: true });
  });
  it("Codex", () => {
    expect(launchArgs("codex", "auto-edit", false)).toEqual({ args: ["--sandbox", "workspace-write", "--ask-for-approval", "on-request"], native: true });
    expect(launchArgs("codex", "bypass", false)).toEqual({ args: ["--dangerously-bypass-approvals-and-sandbox"], native: true });
  });
  it("Antigravity: no Full auto without Marshell's hook, since it has no classifier of its own", () => {
    expect(launchArgs("antigravity", "auto-edit", false)).toEqual({ args: ["--mode", "accept-edits"], native: true });
    expect(launchArgs("antigravity", "bypass", false)).toEqual({ args: ["--dangerously-skip-permissions"], native: true });
    expect(launchArgs("antigravity", "full-auto", false)).toBeNull();
  });
});

describe("decide: what Marshell's hook answers when a CLI asks", () => {
  it("a risky request always gets a card, in every mode, Bypass included (the guard rail)", () => {
    for (const mode of MODES) for (const cli of ["claude", "codex", "antigravity"] as const) expect(decide(cli, mode, command, true), `${cli} ${mode}`).toBe("ask");
  });
  it("Manual and Plan ask about everything", () => {
    for (const mode of ["manual", "plan"] as const) for (const r of [edit, command]) expect(decide("claude", mode, r, false)).toBe("ask");
  });
  it("Auto-edit allows edits inside the project and asks about the rest", () => {
    expect(decide("codex", "auto-edit", edit, false)).toBe("allow");
    expect(decide("codex", "auto-edit", editOutside, false)).toBe("ask");
    expect(decide("codex", "auto-edit", command, false)).toBe("ask");
  });
  it("Full auto: where the CLI's classifier already let the rest through, what reaches Marshell asks; without one, Marshell allows", () => {
    expect(decide("claude", "full-auto", command, false)).toBe("ask");
    expect(decide("codex", "full-auto", command, false)).toBe("ask");
    expect(decide("antigravity", "full-auto", command, false)).toBe("allow");
  });
  it("Bypass allows everything that is not risky", () => {
    for (const cli of ["claude", "codex", "antigravity"] as const) for (const r of [edit, editOutside, command]) expect(decide(cli, "bypass", r, false)).toBe("allow");
  });
});

describe("switchKind: changing mode mid-session", () => {
  it("is instant when the session's native start stays the same (only Marshell's answer changes)", () => {
    expect(switchKind("claude", "manual", "bypass", true)).toBe("instant");
    expect(switchKind("antigravity", "auto-edit", "full-auto", true)).toBe("instant");
  });
  it("needs the CLI's own mode when the native start differs (Plan, or a classifier)", () => {
    expect(switchKind("claude", "manual", "plan", true)).toBe("native");
    expect(switchKind("claude", "bypass", "full-auto", true)).toBe("native");
    expect(switchKind("codex", "plan", "manual", true)).toBe("instant");
  });
  it("is always native with the hooks off", () => {
    expect(switchKind("claude", "manual", "auto-edit", false)).toBe("native");
  });
  it("is unavailable when the CLI can't have the target mode", () => {
    expect(switchKind("antigravity", "manual", "full-auto", false)).toBe("unavailable");
  });
});

describe("effectiveMode: a CLI setting that skips every prompt", () => {
  it("makes the session Bypass whatever mode was picked", () => {
    for (const mode of MODES) expect(effectiveMode(mode, true), mode).toBe("bypass");
  });
  it("leaves the mode alone otherwise", () => {
    for (const mode of MODES) expect(effectiveMode(mode, false), mode).toBe(mode);
  });
});
