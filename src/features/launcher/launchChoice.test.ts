import { describe, expect, it } from "vitest";
import { AGENT_IDS } from "../../components/AgentMark/agents";
import { TOOL_IDS } from "../../components/AgentMark/tools";
import { PROJECTS } from "./fixtures";
import {
  LAUNCH_GROUPS,
  LAUNCH_PROGRAMS,
  MODES,
  PACES,
  applyRemembered,
  isAgentProgram,
  launcherSentence,
  programById,
  programsIn,
  rememberedNote,
  startingText,
  type LauncherChoice,
} from "./launchChoice";

const base: LauncherChoice = { program: "claude", projectId: "my-app", mode: "Plan", pace: "Quick", worktree: false };

describe("launcherSentence", () => {
  it("reads as one sentence", () => {
    expect(launcherSentence(base, PROJECTS)).toBe("Start Claude in my-app on main, Plan mode, Quick, no worktree");
  });
  it("says worktree when it is on, and drops the branch when there is none", () => {
    expect(launcherSentence({ ...base, worktree: true, projectId: "scratch" }, PROJECTS)).toBe("Start Claude in scratch, Plan mode, Quick, in a new worktree");
  });
  it("copes with a project that is not in the list", () => {
    expect(launcherSentence({ ...base, projectId: "gone" }, PROJECTS)).toContain("in an unknown folder");
  });
  it("is shorter for a manual CLI: no mode, no pace, no worktree", () => {
    expect(launcherSentence({ ...base, program: "powershell" }, PROJECTS)).toBe("Start PowerShell in my-app on main");
    expect(launcherSentence({ ...base, program: "ssh", projectId: "scratch" }, PROJECTS)).toBe("Start SSH in scratch");
  });
});

describe("options", () => {
  it("has the modes and paces the sentence uses", () => {
    expect(MODES).toEqual(["Manual", "Plan", "Auto-edit"]);
    expect(PACES).toEqual(["Quick", "Balanced", "Deep"]);
  });
});

describe("programs the first chip offers", () => {
  it("groups them as Agents, Shells and Tools, in that order", () => {
    expect(LAUNCH_GROUPS).toEqual(["Agents", "Shells", "Tools"]);
    for (const g of LAUNCH_GROUPS) expect(programsIn(g).length, g).toBeGreaterThan(0);
    expect(LAUNCH_PROGRAMS.map((p) => p.group)).toEqual([...LAUNCH_PROGRAMS.map((p) => p.group)].sort((a, b) => LAUNCH_GROUPS.indexOf(a) - LAUNCH_GROUPS.indexOf(b)));
  });
  it("offers AI agents under Agents and only manual CLIs under Shells and Tools", () => {
    for (const p of programsIn("Agents")) expect(AGENT_IDS as readonly string[], p.id).toContain(p.id);
    for (const g of ["Shells", "Tools"] as const) for (const p of programsIn(g)) expect(TOOL_IDS as readonly string[], p.id).toContain(p.id);
  });
  it("covers the shells and a spread of dev, remote and cloud tools, with a catch-all", () => {
    const ids = LAUNCH_PROGRAMS.map((p) => p.id);
    for (const id of ["claude", "codex", "antigravity", "copilot", "cursor", "opencode", "amp", "qwen", "grok", "powershell", "bash", "zsh", "cmd", "gitbash", "wsl", "python", "node", "docker", "kubectl", "terraform", "ssh", "aws", "program"]) expect(ids, id).toContain(id);
  });
  it("lists Google's one agent, Antigravity: Gemini CLI is dropped", () => {
    expect(LAUNCH_PROGRAMS.filter((p) => p.group === "Agents").map((p) => p.label)).toEqual(["Claude", "Codex", "Antigravity", "GitHub Copilot", "Cursor", "opencode", "Amp", "Qwen", "Grok"]);
    expect(LAUNCH_PROGRAMS.map((p) => p.id)).not.toContain("gemini");
  });
  it("has unique ids, labels and a first-line hint for each", () => {
    expect(new Set(LAUNCH_PROGRAMS.map((p) => p.id)).size).toBe(LAUNCH_PROGRAMS.length);
    expect(new Set(LAUNCH_PROGRAMS.map((p) => p.label)).size).toBe(LAUNCH_PROGRAMS.length);
    for (const p of LAUNCH_PROGRAMS) expect(p.first.length, p.id).toBeGreaterThan(5);
  });
  it("knows which are agents: only those get mode, pace and worktree chips", () => {
    expect(isAgentProgram("claude")).toBe(true);
    expect(isAgentProgram("powershell")).toBe(false);
    expect(isAgentProgram("program")).toBe(false);
    expect(programById("nope")).toBeUndefined();
  });
});

describe("startingText", () => {
  it("is what the tab shows at once, before the process answers", () => {
    expect(startingText("claude", "my-app")).toBe("Starting Claude Code in my-app…");
    expect(startingText("antigravity", "my-app")).toBe("Starting Antigravity CLI in my-app…");
    expect(startingText("codex", "my-app")).toBe("Starting Codex CLI in my-app…");
    expect(startingText("powershell", "my-app")).toBe("Starting PowerShell in my-app…");
  });
});

describe("remembered choices", () => {
  it("applies what was remembered for the project and leaves the rest", () => {
    expect(applyRemembered(base, { mode: "Auto-edit", worktree: true })).toEqual({ ...base, mode: "Auto-edit", worktree: true });
    expect(applyRemembered(base, undefined)).toEqual(base);
  });
  it("explains what it applied, naming the project", () => {
    expect(rememberedNote("my-app", { mode: "Plan", pace: "Quick" })).toBe("Remembered for my-app: Plan · Quick. Change a choice to update it.");
    expect(rememberedNote("my-app", { worktree: true })).toBe("Remembered for my-app: worktree. Change a choice to update it.");
  });
  it("says nothing when nothing was remembered", () => {
    expect(rememberedNote("my-app", undefined)).toBeNull();
    expect(rememberedNote("my-app", {})).toBeNull();
  });
});
