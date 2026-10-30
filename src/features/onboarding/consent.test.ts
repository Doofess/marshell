import { describe, expect, it } from "vitest";
import { canEdit, CAPABILITIES, consentHeadline, entryCount, opensDiff, removalCopy, skillUndo, type Detection } from "./consent";
import { consentProps } from "../agents/agentsState";
import { SETUP } from "../agents/fixtures";

const claude: Detection = { cli: "claude", installed: true, version: "2.1.296", configPath: "C:\Users\Home Office\.claude\settings.json", configState: "ok" };
const all = { status: true, approve: true, statusline: true, skill: true };

describe("entryCount", () => {
  it("is 11 hook entries for Claude with status and approval on, and the status line adds none", () => {
    expect(entryCount("claude", all)).toBe(11);
    expect(entryCount("claude", { status: true, approve: false, statusline: true, skill: false })).toBe(10);
    expect(entryCount("claude", { status: false, approve: false, statusline: false, skill: false })).toBe(0);
  });
  it("counts only what the CLI can do", () => {
    expect(entryCount("antigravity", all)).toBe(5);
    expect(entryCount("codex", all)).toBe(8);
  });
  it("antigravity_entry_counts_match_the_plan: one PreToolUse and four status events, and the one PreToolUse serves both", () => {
    expect(entryCount("antigravity", all)).toBe(5);
    // Status writes the PreToolUse entry too (its permission answer follows `approve`), so status alone is still five.
    expect(entryCount("antigravity", { status: true, approve: false, statusline: true, skill: false })).toBe(5);
    expect(entryCount("antigravity", { status: false, approve: true, statusline: false, skill: false })).toBe(1);
    expect(entryCount("antigravity", { status: false, approve: false, statusline: true, skill: false })).toBe(0);
  });
});

describe("CAPABILITIES", () => {
  it("offers approval on by default in the live consent card", () => {
    const card = consentProps(SETUP, { preview: null, open: false, busy: false });
    expect(card.offer).toContain("approve");
    expect(card.on.approve).toBe(true);
  });
  it("offers Antigravity all three: four status hooks, its PreToolUse permission hook and its status line (docs/research/2026-10-15-local-install-check.md)", () => {
    expect(CAPABILITIES.antigravity).toEqual({
      status: { available: true, entries: 4 },
      approve: { available: true, entries: 1 },
      statusline: { available: true, entries: 0 },
      skill: { available: true, entries: 0 },
    });
    expect(CAPABILITIES.codex.statusline.available).toBe(false);
  });
  it("carries no Gemini copy: Gemini CLI is dropped and hooksConfig.enabled was its setting", () => {
    for (const caps of Object.values(CAPABILITIES)) for (const c of Object.values(caps)) expect(c.reason ?? "").not.toMatch(/gemini|hooksConfig/i);
  });
});

describe("consentHeadline", () => {
  it("names the count, the status line and the whole path, spaces included", () => {
    expect(consentHeadline(claude, all)).toBe("Add 11 hook entries and chain your status line in C:\Users\Home Office\.claude\settings.json.");
  });
  it("leaves out the status line when it is off", () => {
    expect(consentHeadline(claude, { ...all, statusline: false })).toBe("Add 11 hook entries in C:\Users\Home Office\.claude\settings.json.");
  });
  it("says nothing changes when everything is off", () => {
    expect(consentHeadline(claude, { status: false, approve: false, statusline: false, skill: false })).toBe("Change nothing. Claude Code will show limited status.");
  });
  it("adds the skill sentence for a skill that is not there, or that needs an update", () => {
    const skillPath = String.raw`C:\u\.claude\skills\marshell\SKILL.md`;
    const none = consentHeadline({ ...claude, skill: "none", skillPath }, all);
    expect(none.endsWith(` Add the marshell skill in ${skillPath}.`)).toBe(true);
    expect(consentHeadline({ ...claude, skill: "edited", skillPath }, all).endsWith(` Update the marshell skill in ${skillPath}.`)).toBe(true);
    expect(consentHeadline({ ...claude, skill: "outdated", skillPath }, all)).toContain("Update the marshell skill in");
    expect(consentHeadline({ ...claude, skill: "installed", skillPath }, all)).not.toContain("marshell skill");
  });
  it("adds no skill sentence for a skill Marshell can't touch", () => {
    const h = consentHeadline({ ...claude, skill: "unavailable", skillPath: "/s", skillProblem: "No." }, all);
    expect(h).not.toContain("marshell skill");
    expect(h).toBe(consentHeadline(claude, all));
  });
  it("says only the skill sentence when nothing else is on", () => {
    const off = { status: false, approve: false, statusline: false, skill: true };
    expect(consentHeadline({ ...claude, skill: "none", skillPath: "/s" }, off)).toBe("Add the marshell skill in /s.");
  });
  it("offers the skill for all three CLIs", () => {
    for (const c of Object.values(CAPABILITIES)) expect(c.skill).toEqual({ available: true, entries: 0 });
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
    for (const s of [consentHeadline(claude, all), consentHeadline(claude, { status: false, approve: false, statusline: false, skill: false })]) expect(s).not.toMatch(/!|successfully|oops/i);
  });
});

describe("a status line added rather than chained", () => {
  const none: Detection = { ...claude, hasOwnStatusLine: false };
  const on = { status: true, approve: false, statusline: false, skill: false };
  it("says the status line is added", () => {
    expect(consentHeadline(none, on)).toBe(`Add 10 hook entries and a status line in ${none.configPath}.`);
  });
  it("still chains when the user has one", () => {
    expect(consentHeadline({ ...claude, hasOwnStatusLine: true }, { ...on, statusline: true })).toContain("chain your status line");
  });
  it("removal says it removes what it added", () => {
    expect(removalCopy("claude", on, undefined, undefined, true)).toBe("Marshell removes the hook entries and the status line it added; the rest of the file stays as it is.");
  });
  it("says how the skill is undone, alone or with the hooks", () => {
    const off = { status: false, approve: false, statusline: false, skill: true };
    expect(removalCopy("claude", off, undefined, undefined, false, "add")).toBe("Marshell removes the marshell skill it added.");
    expect(removalCopy("claude", off, undefined, undefined, false, "update")).toBe("Marshell puts the earlier marshell skill back.");
    expect(removalCopy("claude", off)).toBe("Marshell changes nothing, so there is nothing to undo.");
    expect(removalCopy("claude", { ...off, status: true }, undefined, undefined, false, "add")).toBe(
      "Marshell removes the hook entries it added and removes the marshell skill it added; the rest of the file stays as it is.",
    );
  });
  it("maps the switch and the skill state to the undo part and the diff", () => {
    expect(skillUndo(true, "none")).toBe("add");
    expect(skillUndo(true, "outdated")).toBe("update");
    expect(skillUndo(true, "edited")).toBe("update");
    expect(skillUndo(true, "installed")).toBeUndefined();
    expect(skillUndo(false, "none")).toBeUndefined();
    expect(skillUndo(true, undefined)).toBeUndefined();
    expect(opensDiff(true, "edited")).toBe(true);
    expect(opensDiff(false, "edited")).toBe(false);
    expect(opensDiff(true, "outdated")).toBe(false);
    expect(opensDiff(true, undefined)).toBe(false);
    expect(skillUndo(true, "unavailable")).toBeUndefined();
    expect(opensDiff(true, "unavailable")).toBe(false);
  });
});
