import { describe, expect, it, vi } from "vitest";
import type { AgentSetup } from "../../generated/AgentSetup";
import { actions, consentProps, consentQueue, DEFAULT_WANT, hooksOffNotice, installRequest, liveStatusOn, rowSetupStart, setupLine, setupWant } from "./agentsState";
import type { ConfigPreview } from "../../generated/ConfigPreview";
import type { FilePreview } from "../../generated/FilePreview";
import { AGY_SETUP, CODEX_SETUP, PREVIEW, SETUP } from "./fixtures";

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
    const installed = s({ hooks: "installed" });
    const p = consentProps(installed, flow(withFiles([{ path: "C:/h/.claude/settings.json", status: { kind: "noChange" } }], false)));
    expect(p.scenario.plan.headline).toBe("Already set up. Nothing in C:/h/.claude/settings.json changes.");
    expect(p.allow).toBe(false);
    expect(p.done).toBe(true);
    expect(consentProps(installed, flow(withFiles([], false))).done).toBe(true);
  });
  it("keeps Skip and says nothing will change on a fresh setup with every switch off", () => {
    const p = consentProps(SETUP, { ...flow(withFiles([], false)), want: { status: false, statusline: false, approve: false } });
    expect(p.scenario.plan.headline).toBe("Nothing will change: status is off.");
    expect(p.done).toBe(false);
    expect(p.allow).toBe(false);
  });
  it("offers Allow for a preview that can apply", () => {
    const p = consentProps(SETUP, flow(PREVIEW));
    expect(p.allow).toBe(true);
    expect(p.done).toBe(false);
    expect(p.scenario.plan.headline).toBeUndefined();
  });
  // Set up again on a Partial install changes only what is missing or wrong, so the whole install's count would mislead.
  it("calls Set up again on a Partial install a repair, not the whole install", () => {
    const p = consentProps(s({ hooks: "partial" }), flow(PREVIEW));
    expect(p.scenario.plan.headline).toBe("Repair the hook entries Marshell added in C:/h/.claude/settings.json.");
    expect(p.allow).toBe(true);
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
    expect(installRequest(setupWant(s({ hooks: "installed", approve_installed: false })))).toEqual({ status: false, statusline: false, approve: true });
    expect(setupWant(SETUP)).toEqual(DEFAULT_WANT);
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

describe("rowSetupStart", () => {
  it("opens the setup of the agent whose row was clicked", () => {
    expect(rowSetupStart("codex")).toEqual({ setup: "codex" });
    expect(rowSetupStart("claude")).toEqual({ setup: "claude" });
  });
});

describe("liveStatusOn", () => {
  it("is on unless the agent is in status_off", () => {
    expect(liveStatusOn([], "claude")).toBe(true);
    expect(liveStatusOn(["codex"], "claude")).toBe(true);
    expect(liveStatusOn(["codex", "claude"], "claude")).toBe(false);
  });
});

describe("Codex", () => {
  const flow = { preview: PREVIEW, open: false, busy: false };
  it("consentProps for Codex asks for trust, names Codex and offers no status line", () => {
    const p = consentProps(CODEX_SETUP, flow);
    expect(p.scenario.detection.cli).toBe("codex");
    expect(p.scenario.plan.cli).toBe("codex");
    expect(p.scenario.plan.needsTrust).toBe(true);
    expect(p.scenario.plan.configPath).toBe("C:/h/.codex/hooks.json");
    expect(p.offer).toEqual(["status", "approve"]);
    expect(p.on.statusline).toBe(false);
    expect(p.scenario.plan.removal).toBe("Marshell removes the hook entries it added; the rest of the file stays as it is.");
    expect(consentProps(SETUP, flow).scenario.plan.needsTrust).toBe(false);
  });
  it("an installed Codex offers approval only", () => {
    expect(consentProps({ ...CODEX_SETUP, hooks: "installed" }, flow).offer).toEqual(["approve"]);
  });
  it("installRequest never asks Codex for a status line", () => {
    expect(installRequest(DEFAULT_WANT, "codex")).toEqual({ status: true, statusline: false, approve: true });
    expect(installRequest(DEFAULT_WANT)).toEqual({ status: true, statusline: true, approve: true });
  });
  it("setupLine names Codex", () => {
    expect(setupLine(CODEX_SETUP)).toBe("Not set up. Sessions show limited status.");
    expect(setupLine({ ...CODEX_SETUP, found: false })).toBe("Codex was not found on this computer.");
    expect(setupLine({ ...CODEX_SETUP, hooks: "orphaned" })).toContain("so Codex reports a failing hook each time one fires");
  });
  it("setupLine claims status for Codex only once the hooks are trusted", () => {
    const on = { ...CODEX_SETUP, hooks: "installed" as const };
    const path = on.settings_path;
    expect(setupLine({ ...on, trust: "trusted" })).toBe(`Hooks on in ${path}. New sessions report status.`);
    expect(setupLine(on)).toBe(`Hooks on in ${path}. New sessions report status.`);
    expect(setupLine({ ...on, trust: "untrusted" })).toBe(`Hooks on in ${path}. New sessions report status once Codex trusts them.`);
    expect(setupLine({ ...on, trust: "changed" })).toBe(`Hooks on in ${path}. New sessions report status once Codex trusts them.`);
  });
  it("onboarding offers one card per found agent with hooks, Claude first", () => {
    expect(consentQueue([SETUP, CODEX_SETUP]).map((x) => x.agent)).toEqual(["claude", "codex"]);
    expect(consentQueue([SETUP, { ...CODEX_SETUP, found: false }]).map((x) => x.agent)).toEqual(["claude"]);
    expect(consentQueue([{ ...SETUP, found: false }, CODEX_SETUP]).map((x) => x.agent)).toEqual(["codex"]);
    expect(consentQueue([{ ...SETUP, hooks: "installed", approve_installed: true }, CODEX_SETUP]).map((x) => x.agent)).toEqual(["codex"]);
  });
});

describe("Antigravity", () => {
  const flow = { preview: PREVIEW, open: false, busy: false };
  it("onboarding_offers_antigravity_when_agy_is_found: its card comes after Claude's and Codex's, and only when it is found", () => {
    expect(consentQueue([SETUP, CODEX_SETUP, AGY_SETUP]).map((x) => x.agent)).toEqual(["claude", "codex", "antigravity"]);
    expect(consentQueue([SETUP, { ...AGY_SETUP, found: false }]).map((x) => x.agent)).toEqual(["claude"]);
    expect(consentQueue([{ ...AGY_SETUP, settings_state: "unparsable" }])).toEqual([]);
  });
  it("consentProps names both files, offers all three switches and no hand-over, and asks for no trust", () => {
    const p = consentProps(AGY_SETUP, flow);
    expect(p.scenario.detection.cli).toBe("antigravity");
    // hooks.json is new unless the preview read one (a file with a hash before).
    expect(p.scenario.detection.hooks).toEqual({ path: "C:/h/.gemini/config/hooks.json", exists: false });
    const existing = { ...PREVIEW, files: [{ ...PREVIEW.files[0]!, path: "C:/h/.gemini/config/hooks.json" }] };
    expect(consentProps(AGY_SETUP, { ...flow, preview: existing }).scenario.detection.hooks?.exists).toBe(true);
    expect(p.scenario.detection.skipsAll).toBeUndefined();
    expect(p.scenario.plan.configPath).toBe("C:/h/.gemini/antigravity-cli/settings.json");
    expect(p.scenario.plan.needsTrust).toBe(false);
    expect(p.offer).toEqual(["status", "approve", "statusline"]);
    expect(consentProps(AGY_SETUP, { preview: null, open: false, busy: true }).scenario.detection.hooks?.exists).toBe(false);
  });
  it("installRequest asks Antigravity for a status line like Claude Code", () => {
    expect(installRequest(DEFAULT_WANT, "antigravity")).toEqual({ status: true, statusline: true, approve: true });
  });
  it("setupLine names hooks.json, not settings.json, for the hooks", () => {
    const on = { ...AGY_SETUP, hooks: "installed" as const };
    expect(setupLine(on)).toBe("Hooks on in C:/h/.gemini/config/hooks.json. New sessions report status.");
    expect(setupLine({ ...on, hooks: "partial" })).toContain("Some hooks are missing in C:/h/.gemini/config/hooks.json.");
    expect(setupLine({ ...on, hooks: "orphaned" })).toContain("Marshell's hooks are in C:/h/.gemini/config/hooks.json");
  });
  it("setupLine says orphaned hooks stop every Antigravity tool call", () => {
    // agy blocks a tool call whose PreToolUse command can't start (6d research, A3), outside Marshell too.
    expect(setupLine({ ...AGY_SETUP, hooks: "orphaned" })).toBe(
      "Marshell's hooks are in C:/h/.gemini/config/hooks.json, but the bridge at C:/m/bin/marshell-hook.exe is missing, so Antigravity CLI blocks every tool call, outside Marshell too. Turn the hooks off, or set up again.",
    );
  });
  it("rowSetupStart opens Antigravity's setup", () => {
    expect(rowSetupStart("antigravity")).toEqual({ setup: "antigravity" });
  });
});
