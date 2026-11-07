import { describe, expect, it } from "vitest";
import { AGENT_IDS } from "../../components/AgentMark/agents";
import { TOOL_IDS } from "../../components/AgentMark/tools";
import { PROJECTS } from "./fixtures";
import {
  LAUNCH_AGENTS,
  LAUNCH_SHELLS,
  MODES,
  PACES,
  applyRemembered,
  adminChip,
  isAgentProgram,
  launcherSentence,
  programById,
  rememberedNote,
  WAITING_FOR_ADMIN,
  type Remembered,
  startingText,
  type LaunchProgram,
  type LauncherChoice,
} from "./launchChoice";

const base: LauncherChoice = { shell: "powershell", agent: "claude", projectId: "my-app", mode: "plan", pace: "Quick", worktree: false };

describe("launcherSentence", () => {
  it("reads as one sentence: the shell, the agent, the folder", () => {
    expect(launcherSentence(base, PROJECTS)).toBe("Start PowerShell running Claude in my-app on main, Plan mode, Quick, no worktree");
  });
  it("says worktree when it is on, and drops the branch when there is none", () => {
    expect(launcherSentence({ ...base, worktree: true, projectId: "scratch" }, PROJECTS)).toBe("Start PowerShell running Claude in scratch, Plan mode, Quick, in a new worktree");
  });
  it("copes with a project that is not in the list", () => {
    expect(launcherSentence({ ...base, projectId: "gone" }, PROJECTS)).toContain("in an unknown folder");
  });
  it("leaves the mode out for an agent that offers none", () => {
    const opencode: LaunchProgram = { id: "opencode", kind: "opencode", label: "opencode", cli: "opencode", group: "Agents", first: "What should opencode do first? (optional)", modes: [] };
    expect(launcherSentence({ ...base, agent: "opencode" }, PROJECTS, [opencode])).toBe("Start PowerShell running opencode in my-app on main, Quick, no worktree");
  });
  it("with_no_agent_the_sentence_reads_with_no_agent: no mode, no pace, and the worktree only when it is on", () => {
    expect(launcherSentence({ ...base, agent: null }, PROJECTS)).toBe("Start PowerShell with no agent in my-app on main");
    expect(launcherSentence({ ...base, shell: "cmd", agent: null, projectId: "scratch" }, PROJECTS)).toBe("Start Command Prompt with no agent in scratch");
    expect(launcherSentence({ ...base, agent: null, worktree: true }, PROJECTS)).toBe("Start PowerShell with no agent in my-app on main, in a new worktree");
  });
});

describe("options", () => {
  it("has the modes and paces the sentence uses", () => {
    expect(MODES).toEqual(["manual", "plan", "auto-edit", "full-auto", "bypass"]);
    expect(MODES as string[]).not.toContain("dont-ask");
    expect(PACES).toEqual(["Quick", "Balanced", "Deep"]);
  });
});

describe("what the shell and agent chips offer", () => {
  it("lists agents only: every one is an AI agent", () => {
    for (const p of LAUNCH_AGENTS) expect(AGENT_IDS as readonly string[], p.id).toContain(p.id);
    expect(LAUNCH_AGENTS.every((p) => p.group === "Agents")).toBe(true);
  });
  it("lists the shells by their tool marks, each with its family", () => {
    for (const s of LAUNCH_SHELLS) expect(TOOL_IDS as readonly string[], s.id).toContain(s.kind);
    expect(LAUNCH_SHELLS.map((s) => s.id)).toEqual(["powershell", "cmd", "gitbash", "wsl", "bash", "zsh"]);
    expect(LAUNCH_SHELLS.find((s) => s.id === "cmd")?.family).toBe("cmd");
  });
  it("covers the agents", () => {
    const ids = LAUNCH_AGENTS.map((p) => p.id);
    for (const id of ["claude", "codex", "antigravity", "copilot", "cursor", "opencode", "amp", "qwen", "grok"]) expect(ids, id).toContain(id);
    for (const id of ["powershell", "bash", "zsh", "cmd", "gitbash", "wsl", "python", "node", "docker", "kubectl", "terraform", "ssh", "aws", "program"]) expect(ids, id).not.toContain(id);
  });
  it("lists Google's one agent, Antigravity: Gemini CLI is dropped", () => {
    expect(LAUNCH_AGENTS.map((p) => p.label)).toEqual(["Claude", "Codex", "Antigravity", "GitHub Copilot", "Cursor", "opencode", "Amp", "Qwen", "Grok"]);
    expect(LAUNCH_AGENTS.map((p) => p.id)).not.toContain("gemini");
  });
  it("has unique ids, labels and a first-line hint for each", () => {
    expect(new Set(LAUNCH_AGENTS.map((p) => p.id)).size).toBe(LAUNCH_AGENTS.length);
    expect(new Set(LAUNCH_AGENTS.map((p) => p.label)).size).toBe(LAUNCH_AGENTS.length);
    for (const p of LAUNCH_AGENTS) expect(p.first.length, p.id).toBeGreaterThan(5);
  });
  it("knows which choices have an agent: only those get mode, pace and prompt", () => {
    expect(isAgentProgram("claude")).toBe(true);
    expect(isAgentProgram(null)).toBe(false);
    expect(programById("profile:nope")).toBeUndefined();
  });
});

describe("startingText", () => {
  it("is what the tab shows at once, before the process answers", () => {
    expect(startingText("claude", "my-app")).toBe("Starting Claude Code in my-app…");
    expect(startingText("antigravity", "my-app")).toBe("Starting Antigravity CLI in my-app…");
    expect(startingText("codex", "my-app")).toBe("Starting Codex CLI in my-app…");
    expect(startingText("PowerShell", "my-app", [])).toBe("Starting PowerShell in my-app…");
  });
});

describe("remembered choices", () => {
  it("applies what was remembered for the project and leaves the rest", () => {
    expect(applyRemembered(base, { mode: "auto-edit" })).toEqual({ ...base, mode: "auto-edit" });
    expect(applyRemembered(base, undefined)).toEqual(base);
  });
  it("falls back to the first mode the program offers when the remembered one is not offered", () => {
    const cursor: LaunchProgram = { id: "cursor", kind: "cursor", label: "Cursor", cli: "Cursor CLI", group: "Agents", first: "What should Cursor do first? (optional)", modes: ["manual", "plan", "bypass"] };
    const opencode: LaunchProgram = { ...cursor, id: "opencode", kind: "opencode", label: "opencode", modes: [] };
    const on = (agent: LaunchProgram["id"]): LauncherChoice => ({ ...base, agent });
    expect(applyRemembered(on("cursor"), { mode: "auto-edit" }, [cursor, opencode])).toEqual({ ...base, agent: "cursor", mode: "manual" });
    expect(applyRemembered(on("cursor"), { mode: "plan" }, [cursor, opencode])).toMatchObject({ mode: "plan" });
    // No modes: the remembered one is kept as data, but nothing sends or shows it.
    expect(applyRemembered(on("opencode"), { mode: "auto-edit" }, [cursor, opencode])).toMatchObject({ mode: "auto-edit" });
    // A program the list lacks, or one with no `modes` field (the five), keeps what was remembered.
    expect(applyRemembered(on("claude"), { mode: "auto-edit" })).toMatchObject({ mode: "auto-edit" });
  });
  it("never brings the worktree flag back: the chip starts off every time", () => {
    // A stale entry that still carries the flag is ignored, and the note does not mention it.
    const stale = { mode: "plan", worktree: true } as Parameters<typeof applyRemembered>[1];
    expect(applyRemembered(base, stale)).toEqual({ ...base, mode: "plan", worktree: false });
    expect(rememberedNote("my-app", stale)).toBe("Remembered for my-app: Plan. Change a choice to update it.");
  });
  it("explains what it applied, naming the project", () => {
    expect(rememberedNote("my-app", { mode: "plan", pace: "Quick" })).toBe("Remembered for my-app: Plan · Quick. Change a choice to update it.");
  });
  it("says nothing when nothing was remembered", () => {
    expect(rememberedNote("my-app", undefined)).toBeNull();
    expect(rememberedNote("my-app", {})).toBeNull();
  });
});

describe("Administrator is launcher state only", () => {
  it("turns a requested Administrator chip off and disables toggling when the agent refuses", () => {
    const reason = "Codex needs an update.";
    expect(adminChip(true, "windows", reason)).toMatchObject({ enabled: false, on: false, reason });
    expect(adminChip(false, "windows", reason)).toMatchObject({ enabled: false, on: false, reason });
    expect(adminChip(true, "windows")).toMatchObject({ enabled: true, on: true });
    expect(adminChip(true, "linux", reason).reason).toBe("Administrator: available on Windows only for now.");
  });
  it("admin_is_never_remembered_for_the_project", () => {
    // Nothing in the choice, the default or the remembered shape can carry it.
    expect(base).not.toHaveProperty("elevated");
    const stored = { mode: "plan", pace: "Quick", elevated: true } as Remembered;
    expect(applyRemembered(base, stored)).not.toHaveProperty("elevated");
    expect(rememberedNote("my-app", stored)).not.toContain("dministrator");
  });
  it("the_new_tab_waits_for_approval: the text, and the sentence naming it", () => {
    expect(WAITING_FOR_ADMIN).toBe("Waiting for administrator approval…");
    expect(launcherSentence(base, PROJECTS, LAUNCH_AGENTS, true)).toContain("as administrator");
    expect(launcherSentence(base, PROJECTS)).not.toContain("administrator");
  });
});
