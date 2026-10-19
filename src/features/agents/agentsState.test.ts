import { describe, expect, it, vi } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { actions, consentProps, DEFAULT_WANT, hooksOffNotice, installRequest, liveStatusOn, setNeedsYouSound, setupLine, setupWant } from "./agentsState";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { FilePreview } from "../../generated/FilePreview";
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
    expect(actions(s({ hooks: "installed", approve_installed: true }))).toEqual(["hooksOff"]);
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

describe("consentProps for a preview that cannot be applied", () => {
  const withFiles = (files: Array<{ path: string; status: FilePreview["status"] }>, can_apply: boolean): ConfigPreview => ({
    ...PREVIEW,
    can_apply,
    files: files.map((f) => ({ ...PREVIEW.files[0]!, diff: "", ...f })),
  });
  const flow = (preview: ConfigPreview) => ({ preview, open: false, busy: false });
  it("names the first refused file and offers no Allow", () => {
    const p = consentProps(SETUP, flow(withFiles([{ path: "C:/x/a.json", status: { kind: "refused", reason: "tooLarge", detail: "it is over 1 MB" } }, { path: "C:/h/.claude/settings.json", status: { kind: "changes" } }], false)));
    expect(p.scenario.plan.headline).toBe("Marshell will not edit C:/x/a.json: it is over 1 MB");
    expect(p.allow).toBe(false);
    expect(p.done).toBe(false);
  });
  it("says it is already set up when nothing changes, and turns Skip into Done", () => {
    const p = consentProps(SETUP, flow(withFiles([{ path: "C:/h/.claude/settings.json", status: { kind: "noChange" } }], false)));
    expect(p.scenario.plan.headline).toBe("Already set up. Nothing in C:/h/.claude/settings.json changes.");
    expect(p.allow).toBe(false);
    expect(p.done).toBe(true);
    expect(consentProps(SETUP, flow(withFiles([], false))).done).toBe(true);
  });
  it("offers Allow for a preview that can apply", () => {
    const p = consentProps(SETUP, flow(PREVIEW));
    expect(p.allow).toBe(true);
    expect(p.done).toBe(false);
    expect(p.scenario.plan.headline).toBeUndefined();
  });
  it("offers no Allow without a preview", () => {
    expect(consentProps(SETUP, { preview: null, open: false, busy: false }).allow).toBe(false);
  });
});

describe("consentProps", () => {
  it("builds the card's input from the core's setup and preview", () => {
    const p = consentProps(SETUP, { preview: PREVIEW, open: true, busy: false });
    expect(p.scenario.detection.configPath).toBe("C:/h/.claude/settings.json");
    expect(p.scenario.plan.diff).toContain('"hooks": {}');
    expect(p.scenario.plan.backupPath).toBe("C:/m/snapshots");
    expect(p.scenario.plan.chainPath).toBe("C:/m/agents/claude/statusline-chain.json");
    expect(p.offer).toEqual(["status", "approve", "statusline"]);
    expect(p.on).toEqual({ status: true, approve: true, statusline: true });
  });
  it("never describes a refused file as about to change", () => {
    const p = consentProps(SETUP, { preview: null, open: false, busy: false, error: "hooks is not an object" });
    expect(p.error).toBe("hooks is not an object");
    expect(p.scenario.plan.headline).toBe("Marshell will not edit C:/h/.claude/settings.json, so nothing will change.");
    expect(p.scenario.plan.diff).toBe("");
  });
});

describe("the switches the person sets", () => {
  it("starts an upgrade with only approval requested", () => {
    expect(setupWant(s({ hooks: "installed", approve_installed: false }))).toEqual({ status: false, statusline: false, approve: true });
    expect(setupWant(SETUP)).toEqual(DEFAULT_WANT);
  });
  it("writes the inverse sound setting and returns the saved state", async () => {
    const sync = { soundOff: false, setSoundOff: vi.fn(async (off: boolean) => { sync.soundOff = off; }) };
    expect(await setNeedsYouSound(sync, false)).toBe(false);
    expect(sync.setSoundOff).toHaveBeenLastCalledWith(true);
    expect(await setNeedsYouSound(sync, true)).toBe(true);
    expect(sync.setSoundOff).toHaveBeenLastCalledWith(false);
  });
  it("propagates a failed sound save instead of reporting a new state", async () => {
    const sync = { soundOff: false, setSoundOff: vi.fn().mockRejectedValue(new Error("save failed")) };
    await expect(setNeedsYouSound(sync, false)).rejects.toThrow("save failed");
  });
  it("sends the approval switch in the hook plan request", () => {
    expect(installRequest({ ...DEFAULT_WANT, approve: true }).approve).toBe(true);
    expect(installRequest({ ...DEFAULT_WANT, approve: false }).approve).toBe(false);
  });
  it("offers only approval for an existing status install", () => {
    const p = consentProps(s({ hooks: "installed", approve_installed: false }), {
      preview: PREVIEW, open: true, busy: false,
      want: { status: false, statusline: false, approve: true },
    });
    expect(p.offer).toEqual(["approve"]);
    expect(p.on).toEqual({ status: false, statusline: false, approve: true });
    expect(p.scenario.plan.diff).toContain('"hooks": {}');
  });
  it("start with status and the status line on", () => {
    expect(installRequest(DEFAULT_WANT)).toEqual({ status: true, statusline: true, approve: true });
  });
  it("send statusline:false when that switch is off, so the status line is never touched", () => {
    expect(installRequest({ status: true, statusline: false, approve: false })).toEqual({ status: true, statusline: false, approve: false });
  });
  it("never ask for the status line when status is off", () => {
    expect(installRequest({ status: false, statusline: true, approve: false })).toEqual({ status: false, statusline: false, approve: false });
  });
  it("show the card's switches as the person set them", () => {
    const p = consentProps(SETUP, { preview: PREVIEW, open: false, busy: false, want: { status: true, statusline: false, approve: false } });
    expect(p.on).toEqual({ status: true, approve: false, statusline: false });
    expect(p.scenario.plan.headline).toBeUndefined();
    expect(p.scenario.plan.chainPath).toBe("C:/m/agents/claude/statusline-chain.json");
  });
  it("wait for a preview when both capabilities are off", () => {
    const p = consentProps(SETUP, { preview: null, open: false, busy: false, want: { status: false, statusline: true, approve: false } });
    expect(p.on.status).toBe(false);
    expect(p.scenario.plan.headline).toBe("Checking C:/h/.claude/settings.json…");
  });
});

describe("liveStatusOn", () => {
  it("is on unless the agent is in status_off", () => {
    expect(liveStatusOn([], "claude")).toBe(true);
    expect(liveStatusOn(["codex"], "claude")).toBe(true);
    expect(liveStatusOn(["codex", "claude"], "claude")).toBe(false);
  });
});
