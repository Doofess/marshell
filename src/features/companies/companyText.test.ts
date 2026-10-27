import { describe, expect, it } from "vitest";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import { fileLine, manualUpdateLine, newerLine, runningNote, versionLine } from "./companyText";

const install = (over: Partial<AgentInstall> = {}): AgentInstall => ({ agent: "claude", found: true, version: "2.1.0", install: { kind: "native" }, update_command: "claude update", newer_version: null, ...over });
const view = (state: AgentSettingsView["state"]): AgentSettingsView => ({ agent: "claude", path: "C:/u/.claude/settings.json", state, curated: [], others: [], raw: null, editable: false, hash: null, text: null, raw_lock: null });

describe("companyText", () => {
  it("versionLine", () => {
    expect(versionLine(install())).toBe("Version 2.1.0");
    expect(versionLine(install({ version: null }))).toBe("Version –");
  });
  it("newerLine", () => {
    expect(newerLine(install())).toBeNull();
    expect(newerLine(install({ newer_version: "0.9.1" }))).toBe("Version 0.9.1 is available.");
  });
  it("manualUpdateLine", () => {
    expect(manualUpdateLine("claude", "Claude Code")).toBe("Marshell can't tell how Claude Code was installed. Update it with claude update, or npm install -g @anthropic-ai/claude-code@latest for an npm install.");
    expect(manualUpdateLine("codex", "Codex")).toBe("Marshell can't tell how Codex was installed. Update it with codex update, or npm install -g @openai/codex@latest for an npm install.");
    expect(manualUpdateLine("antigravity", "Antigravity")).toBe("Marshell can't tell how Antigravity was installed. Update it with agy update.");
  });
  it("runningNote", () => {
    expect(runningNote("Claude Code", 0)).toBeNull();
    expect(runningNote("Claude Code", 1)).toBe("Claude Code is running in 1 tab. The update may fail while it runs.");
    expect(runningNote("Claude Code", 3)).toBe("Claude Code is running in 3 tabs. The update may fail while they run.");
  });
  it("fileLine", () => {
    expect(fileLine(view({ kind: "missing" }))).toBe("C:/u/.claude/settings.json doesn't exist yet, so every setting has its default.");
    expect(fileLine(view({ kind: "unparsable", line: 4 }))).toBe("C:/u/.claude/settings.json doesn't parse (line 4), so Marshell shows nothing from it.");
    expect(fileLine(view({ kind: "unreadable" }))).toBe("Marshell can't read C:/u/.claude/settings.json.");
    expect(fileLine(view({ kind: "ok" }))).toBeNull();
  });
});
