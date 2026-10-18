import { describe, expect, it } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { actions, consentProps, DEFAULT_WANT, hooksOffNotice, installRequest, setupLine } from "./agentsState";
import { PREVIEW, SETUP } from "./fixtures";

const s = (patch: Partial<AgentSetup>): AgentSetup => ({ ...SETUP, ...patch });

describe("setupLine", () => {
  it("says each state in the plan's words", () => {
    expect(setupLine(s({ hooks: "none" }))).toBe("Not set up. Sessions show limited status.");
    expect(setupLine(s({ hooks: "installed" }))).toBe("Hooks on in C:/h/.claude/settings.json. New sessions report status.");
    expect(setupLine(s({ hooks: "partial" }))).toBe("Some hooks are missing in C:/h/.claude/settings.json. Set up again to repair them.");
    expect(setupLine(s({ hooks: "orphaned" }))).toBe(
      "Marshell's hooks are in C:/h/.claude/settings.json, but the bridge at C:/m/bin/marshell-hook.exe is missing, so Claude Code reports a failing hook each time one fires. Turn the hooks off, or set up again.",
    );
  });
  it("gives the consent headline for a file that does not parse", () => {
    expect(setupLine(s({ settings_state: "unparsable", parse_error: { line: 4, message: "x" } }))).toBe("C:/h/.claude/settings.json does not parse (line 4), so Marshell will not edit it.");
  });
  it("says when Claude Code was not found", () => {
    expect(setupLine(s({ found: false }))).toBe("Claude Code was not found on this computer.");
  });
});

describe("actions", () => {
  it("offers what each state allows", () => {
    expect(actions(s({ hooks: "none" }))).toEqual(["setup"]);
    expect(actions(s({ hooks: "installed" }))).toEqual(["hooksOff"]);
    expect(actions(s({ hooks: "partial" }))).toEqual(["setup", "hooksOff"]);
    expect(actions(s({ hooks: "orphaned" }))).toEqual(["setup", "hooksOff"]);
  });
  it("offers nothing when Claude Code is not found or its file does not parse", () => {
    expect(actions(s({ found: false }))).toEqual([]);
    expect(actions(s({ settings_state: "unparsable" }))).toEqual([]);
  });
});

describe("hooksOffNotice", () => {
  it("adds the status line sentence only when it was left", () => {
    expect(hooksOffNotice(false)).toBe("Hooks off. Sessions started from now show limited status.");
    expect(hooksOffNotice(true)).toBe("Hooks off. Sessions started from now show limited status. Your status line was changed since setup, so it was left as it is.");
  });
});

describe("consentProps", () => {
  it("builds the card's input from the core's setup and preview", () => {
    const p = consentProps(SETUP, { preview: PREVIEW, open: true, busy: false });
    expect(p.scenario.detection.configPath).toBe("C:/h/.claude/settings.json");
    expect(p.scenario.plan.diff).toContain('"hooks": {}');
    expect(p.scenario.plan.backupPath).toBe("C:/m/snapshots");
    expect(p.scenario.plan.chainPath).toBe("C:/m/agents/claude/statusline-chain.json");
    expect(p.offer).toEqual(["status", "statusline"]);
    expect(p.on).toEqual({ status: true, approve: false, statusline: true });
  });
  it("never describes a refused file as about to change", () => {
    const p = consentProps(SETUP, { preview: null, open: false, busy: false, error: "hooks is not an object" });
    expect(p.error).toBe("hooks is not an object");
    expect(p.scenario.plan.headline).toBe("Marshell will not edit C:/h/.claude/settings.json, so nothing will change.");
    expect(p.scenario.plan.diff).toBe("");
  });
});

describe("the switches the person sets", () => {
  it("start with status and the status line on", () => {
    expect(installRequest(DEFAULT_WANT)).toEqual({ status: true, statusline: true });
  });
  it("send statusline:false when that switch is off, so the status line is never touched", () => {
    expect(installRequest({ status: true, statusline: false })).toEqual({ status: true, statusline: false });
  });
  it("never ask for the status line when status is off", () => {
    expect(installRequest({ status: false, statusline: true })).toEqual({ status: false, statusline: false });
  });
  it("show the card's switches as the person set them", () => {
    const p = consentProps(SETUP, { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false } });
    expect(p.on).toEqual({ status: true, approve: false, statusline: false });
    expect(p.scenario.plan.headline).toBeUndefined();
    expect(p.scenario.plan.chainPath).toBe("C:/m/agents/claude/statusline-chain.json");
  });
  it("say nothing will change when status is off", () => {
    const p = consentProps(SETUP, { preview: null, open: false, busy: false, want: { status: false, statusline: true } });
    expect(p.on.status).toBe(false);
    expect(p.scenario.plan.headline).toBe("Nothing will change: status is off.");
  });
});
