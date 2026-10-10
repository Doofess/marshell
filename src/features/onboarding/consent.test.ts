import { describe, expect, it } from "vitest";
import { canEdit, CAPABILITIES, consentHeadline, entryCount, type Detection } from "./consent";

const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", configPath: "C:\Users\Home Office\.claude\settings.json", configState: "ok" };
const all = { status: true, approve: true, statusline: true };

describe("entryCount", () => {
  it("is 9 hook entries for Claude with status and approval on, and the status line adds none", () => {
    expect(entryCount("claude", all)).toBe(9);
    expect(entryCount("claude", { status: true, approve: false, statusline: true })).toBe(8);
    expect(entryCount("claude", { status: false, approve: false, statusline: false })).toBe(0);
  });
  it("counts only what the CLI can do", () => {
    expect(entryCount("antigravity", all)).toBe(0);
    expect(entryCount("codex", all)).toBe(7);
  });
});

describe("CAPABILITIES", () => {
  it("offers Antigravity only its status line, and says the rest is verified when the adapter is built", () => {
    expect(CAPABILITIES.antigravity.statusline).toMatchObject({ available: true, entries: 0 });
    expect(CAPABILITIES.antigravity.status.available).toBe(false);
    expect(CAPABILITIES.antigravity.status.reason).toContain("verified when the adapter is built");
    expect(CAPABILITIES.antigravity.approve).toMatchObject({ available: false, reason: "Antigravity has no verified approval hook, so Marshell focuses the tab instead (verified when the adapter is built)." });
    expect(CAPABILITIES.codex.statusline.available).toBe(false);
  });
  it("carries no Gemini copy: Gemini CLI is dropped and hooksConfig.enabled was its setting", () => {
    for (const caps of Object.values(CAPABILITIES)) for (const c of Object.values(caps)) expect(c.reason ?? "").not.toMatch(/gemini|hooksConfig/i);
  });
});

describe("consentHeadline", () => {
  it("names the count, the status line and the whole path, spaces included", () => {
    expect(consentHeadline(claude, all)).toBe("Add 9 hook entries and chain your status line in C:\Users\Home Office\.claude\settings.json.");
  });
  it("leaves out the status line when it is off", () => {
    expect(consentHeadline(claude, { ...all, statusline: false })).toBe("Add 9 hook entries in C:\Users\Home Office\.claude\settings.json.");
  });
  it("says nothing changes when everything is off", () => {
    expect(consentHeadline(claude, { status: false, approve: false, statusline: false })).toBe("Change nothing. Claude Code will show limited status.");
  });
  it("fails closed on a file that does not parse, naming the line", () => {
    const d: Detection = { ...claude, configState: "unparsable", parseError: { line: 14, message: "Unexpected token }" } };
    expect(consentHeadline(d, all)).toBe("C:\Users\Home Office\.claude\settings.json does not parse (line 14), so Marshell will not edit it.");
    expect(canEdit(d)).toEqual({ ok: false, reason: "The file does not parse." });
  });
  it("never offers to edit a read-only file, and a managed file does not stop the user-level edit", () => {
    expect(canEdit({ ...claude, configState: "read-only" })).toEqual({ ok: false, reason: "The file is read-only." });
    expect(canEdit({ ...claude, managed: { path: "C:\Program Files\ClaudeCode\managed-settings.json" } }).ok).toBe(true);
  });
  it("follows the copy rules: no exclamation marks, no 'successfully', no 'Oops'", () => {
    for (const s of [consentHeadline(claude, all), consentHeadline(claude, { status: false, approve: false, statusline: false })]) expect(s).not.toMatch(/!|successfully|oops/i);
  });
});
