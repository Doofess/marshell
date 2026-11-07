import { describe, expect, it, vi } from "vitest";
import type { Task } from "../../generated/Task";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { Project } from "../launcher/filter";
import { LAUNCH_AGENTS, startingText, type LauncherChoice, type ProgramId } from "../launcher/launchChoice";
import { ApiError } from "../../lib/api/client";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { FolderInfo } from "../../generated/FolderInfo";
import type { AgentCatalog } from "../../generated/AgentCatalog";
import type { CatalogAgent } from "../../generated/CatalogAgent";
import { browseFolder, checkState, chooseFolder, latestOnly, samePath, createRequest, createThenLaunch, focusStateOf, initialFocus, launchRequest, lostFocus, nameSource, preselect, preselectMode, progressText, refocus, rememberTasks, liveAgents, liveShells, replansAfter, startFailure, startProblem, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

const profile = (id: string, family: Profile["family"], name = id): Profile => ({ id, name, command: [id], family, cwd: null, env: [], shell_integration: true, detected: true });
const PROFILES: Profile[] = [profile("pwsh", "power_shell", "PowerShell 7"), profile("cmd", "cmd", "Command Prompt"), profile("git-bash", "posix", "Git Bash"), profile("wsl-ubuntu", "wsl", "Ubuntu")];
const DOC: ProfilesDoc = { profiles: PROFILES, default_id: "pwsh" };
const PROJECTS: Project[] = [{ id: "C:/dev/my-app", name: "my-app", branch: "main", path: "C:/dev/my-app", lastUsedMs: 1 }];
const SIZE = { cols: 120, rows: 30 };
/** `profile:<id>` is Just that shell; anything else is that agent in the default shell. */
const choice = (program: ProgramId): LauncherChoice => ({
  ...(program.startsWith("profile:") ? { shell: program.slice(8), agent: null } : { shell: "pwsh", agent: program }),
  projectId: "C:/dev/my-app",
  mode: "manual",
  pace: "Balanced",
  worktree: false,
});

const catalogAgent = (over: Partial<CatalogAgent> & Pick<CatalogAgent, "id" | "name">): CatalogAgent => ({
  cli: over.name,
  mark: over.id,
  adapter: false,
  own: false,
  found: true,
  version: null,
  prompt: { style: "positional" },
  modes: [],
  can_continue: true,
  ...over,
});
const ALL_MODES: CatalogAgent["modes"] = ["bypass", "manual", "full-auto", "plan", "auto-edit"];
const adapter = (id: string, name: string): CatalogAgent => catalogAgent({ id, name, adapter: true, modes: ALL_MODES });
const DEEPSEEK = { id: "deepseek", name: "DeepSeek", agent: "claude", mark: "deepseek", problem: null };
const catalogWith = (agents: CatalogAgent[], presets: AgentCatalog["presets"] = [DEEPSEEK]): AgentCatalog => ({ agents, presets });
const CATALOG = catalogWith([
  adapter("claude", "Claude Code"),
  adapter("codex", "Codex"),
  adapter("antigravity", "Antigravity"),
  adapter("qwen", "Qwen Code"),
  catalogAgent({ id: "grok", name: "Grok", cli: "Grok Build", modes: ALL_MODES }),
  catalogAgent({ id: "cursor", name: "Cursor", modes: ["plan", "bypass", "manual"] }),
  catalogAgent({ id: "opencode", name: "opencode", modes: [], prompt: { style: "flag", flag: "--prompt" } }),
  catalogAgent({ id: "amp", name: "Amp", prompt: { style: "none" }, modes: ["bypass"] }),
  catalogAgent({ id: "kimi", name: "Kimi", prompt: { style: "none" }, modes: ["manual", "plan", "full-auto", "bypass"] }),
  catalogAgent({ id: "aider", name: "Aider", found: false, prompt: { style: "none" }, modes: ["manual", "bypass"] }),
  catalogAgent({ id: "crush", name: "Crush", mark: "generic", prompt: { style: "none" }, modes: ["manual", "bypass"] }),
]);

describe("liveAgents from the catalog", () => {
  const programs = liveAgents(CATALOG);
  const byId = (id: string) => programs.find((p) => p.id === id)!;
  it("lists every catalog agent in the catalog's order, then the preset: the shell's own answer says which it has", () => {
    expect(programs.map((p) => p.id)).toEqual(["claude", "codex", "antigravity", "qwen", "grok", "cursor", "opencode", "amp", "kimi", "aider", "crush", "preset:deepseek"]);
  });
  it("labels a manifest agent by its catalog name and CLI, and keeps the adapters' own labels", () => {
    expect(byId("grok")).toMatchObject({ label: "Grok", cli: "Grok Build", group: "Agents", kind: "grok" });
    expect(byId("claude")).toMatchObject({ label: "Claude", cli: "Claude Code" });
    expect(byId("antigravity")).toMatchObject({ label: "Antigravity", cli: "Antigravity CLI" });
  });
  it("hides the first-prompt box for a CLI that takes no first prompt, and only for those", () => {
    for (const id of ["kimi", "crush", "amp"]) expect(byId(id).noFirst, id).toBe(true);
    for (const id of ["claude", "grok", "cursor", "opencode"]) expect(byId(id).noFirst, id).toBeUndefined();
    const aider = liveAgents(catalogWith([catalogAgent({ id: "aider", name: "Aider", prompt: { style: "none" } })], []));
    expect(aider[0]!.noFirst).toBe(true);
  });
  it("offers only the modes the manifest maps, in MODES order, and none for opencode", () => {
    expect(byId("cursor").modes).toEqual(["manual", "plan", "bypass"]);
    expect(byId("grok").modes).toEqual(["manual", "plan", "auto-edit", "full-auto", "bypass"]);
    expect(byId("amp").modes).toEqual(["bypass"]);
    expect(byId("opencode").modes).toEqual([]);
    expect(byId("claude").modes).toEqual(["manual", "plan", "auto-edit", "full-auto", "bypass"]);
  });
  it("marks an agent with its own mark, or the generic one when the catalog names a mark the app has none for", () => {
    expect(byId("grok").kind).toBe("grok");
    expect(byId("crush").kind).toBe("generic");
    const own = liveAgents(catalogWith([catalogAgent({ id: "mine", name: "Mine", mark: "no-such-mark", own: true })], []));
    expect(own[0]).toMatchObject({ id: "mine", kind: "generic", label: "Mine" });
  });
  it("lists the DeepSeek preset with its problem, started through Claude Code", () => {
    const p = liveAgents(catalogWith(CATALOG.agents, [{ ...DEEPSEEK, problem: "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek." }])).find((x) => x.id === "preset:deepseek")!;
    expect(p).toMatchObject({ id: "preset:deepseek", label: "DeepSeek", cli: "DeepSeek", kind: "deepseek", group: "Agents", preset: "deepseek", agent: "claude", problem: "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek." });
    expect(byId("preset:deepseek").problem).toBeUndefined();
    expect(byId("preset:deepseek").modes).toEqual(byId("claude").modes);
  });
  it("leaves the preset out when the agent it starts is not in the catalog", () => {
    const noClaude = catalogWith(CATALOG.agents.filter((a) => a.id !== "claude"));
    expect(liveAgents(noClaude).map((p) => p.id)).not.toContain("preset:deepseek");
  });
  it("a catalog that could not be read lists the three adapters as before and claims nothing about the rest", () => {
    const fallback = liveAgents(null);
    expect(fallback.map((p) => p.id)).toEqual(["claude", "codex", "antigravity"]);
    expect(fallback[0]!.modes).toBeUndefined();
  });
});

describe("liveShells", () => {
  it("lists one shell per profile, by its id and name, with the mark and family of the profile", () => {
    const shells = liveShells(PROFILES);
    expect(shells.map((s) => s.id)).toEqual(["pwsh", "cmd", "git-bash", "wsl-ubuntu"]);
    expect(shells.map((s) => s.kind)).toEqual(["powershell", "cmd", "gitbash", "wsl"]);
    expect(shells.map((s) => s.label)).toEqual(["PowerShell 7", "Command Prompt", "Git Bash", "Ubuntu"]);
    expect(shells.map((s) => s.family)).toEqual(["power_shell", "cmd", "posix", "wsl"]);
    expect(liveShells([])).toEqual([]);
  });
});

describe("starting from the catalog's programs", () => {
  const programs = liveAgents(CATALOG);
  const withProblem = liveAgents(catalogWith(CATALOG.agents, [{ ...DEEPSEEK, problem: "Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek." }]));
  it("the preset sends Claude Code with the preset, and the key is not part of the request", () => {
    const req = launchRequest(choice("preset:deepseek"), "fix it", programs, DOC, PROJECTS, SIZE);
    expect(req).toEqual({ agent: "claude", preset: "deepseek", profile_id: "pwsh", cwd: "C:/dev/my-app", mode: "manual", prompt: "fix it", cols: 120, rows: 30, elevated: false });
    expect(JSON.stringify(req)).not.toMatch(/key|token/i);
  });
  it("a preset that can't start shows its problem, and a program that can has none", () => {
    expect(startProblem(choice("preset:deepseek"), withProblem)).toBe("Set DEEPSEEK_API_KEY, then restart Marshell to start DeepSeek.");
    expect(startProblem(choice("preset:deepseek"), programs)).toBeUndefined();
    expect(startProblem(choice("claude"), withProblem)).toBeUndefined();
    expect(startProblem(choice("profile:cmd"), withProblem)).toBeUndefined();
  });
  it("sends no prompt to a CLI that takes none, even with text typed before switching to it", () => {
    for (const id of ["kimi", "amp", "crush"]) expect(launchRequest(choice(id), "fix it", programs, DOC, PROJECTS, SIZE), id).not.toHaveProperty("prompt");
    expect(launchRequest(choice("grok"), "fix it", programs, DOC, PROJECTS, SIZE).prompt).toBe("fix it");
  });
  it("sends a mode the manifest maps, falls back to the first one it maps, and sends none when it maps none", () => {
    expect(launchRequest({ ...choice("grok"), mode: "plan" }, "", programs, DOC, PROJECTS, SIZE).mode).toBe("plan");
    expect(launchRequest({ ...choice("cursor"), mode: "auto-edit" }, "", programs, DOC, PROJECTS, SIZE).mode).toBe("manual");
    expect(launchRequest({ ...choice("amp"), mode: "manual" }, "", programs, DOC, PROJECTS, SIZE).mode).toBe("bypass");
    const opencode = launchRequest({ ...choice("opencode"), mode: "plan" }, "", programs, DOC, PROJECTS, SIZE);
    expect(opencode).not.toHaveProperty("mode");
    expect(opencode.agent).toBe("opencode");
  });
  it("sends the adapters exactly what it did", () => {
    expect(launchRequest({ ...choice("codex"), mode: "plan" }, "go", programs, DOC, PROJECTS, SIZE)).toEqual({ agent: "codex", profile_id: "pwsh", cwd: "C:/dev/my-app", mode: "plan", prompt: "go", cols: 120, rows: 30, elevated: false });
  });
});

describe("preselect", () => {
  const shells = liveShells(PROFILES);
  const docWith = (launcher: LayoutDoc["launcher"]): LayoutDoc => ({ tabs: [], view: {} as LayoutDoc["view"], launcher });
  const entry = (profile_id: string, agent: string | null, preset?: string) => ({ profile_id, agent, seq: 1, mode: null, ...(preset === undefined ? {} : { preset }) });
  it("maps what was remembered to its shell and agent, and nothing to nothing", () => {
    const doc = docWith({ "C:/shell": entry("cmd", null), "C:/agent": entry("wsl-ubuntu", "codex") });
    expect(preselect(doc, "C:/shell", shells, "pwsh")).toEqual({ shell: "cmd", agent: null });
    expect(preselect(doc, "C:/agent", shells, "pwsh")).toEqual({ shell: "wsl-ubuntu", agent: "codex" });
    expect(preselect(doc, "C:/never-used", shells, "pwsh")).toBeUndefined();
    expect(preselect(null, "C:/shell", shells, "pwsh")).toBeUndefined();
  });
  it("preselect_falls_back_when_the_remembered_shell_is_gone", () => {
    const doc = docWith({ "C:/gone": entry("removed", "claude") });
    expect(preselect(doc, "C:/gone", shells, "pwsh")).toEqual({ shell: "pwsh", agent: "claude" });
  });
  it("a_remembered_preset_preselects_the_preset_agent", () => {
    const doc = docWith({ "C:/a": entry("pwsh", "claude", "deepseek") });
    expect(preselect(doc, "C:/a", shells, "pwsh")).toEqual({ shell: "pwsh", agent: "preset:deepseek" });
  });
});

describe("launching in the chosen shell", () => {
  const programs = liveAgents(CATALOG);
  it("an_agent_launch_sends_the_chosen_shell", () => {
    const req = launchRequest({ ...choice("codex"), shell: "wsl-ubuntu" }, "go", programs, DOC, PROJECTS, SIZE);
    expect(req).toEqual({ agent: "codex", profile_id: "wsl-ubuntu", cwd: "C:/dev/my-app", mode: "manual", prompt: "go", cols: 120, rows: 30, elevated: false });
  });
  it("just_the_shell_sends_only_the_profile", () => {
    expect(launchRequest({ ...choice("profile:git-bash"), mode: "plan" }, "dir", programs, DOC, PROJECTS, SIZE)).toEqual({ profile_id: "git-bash", cwd: "C:/dev/my-app", cols: 120, rows: 30, elevated: false });
  });
  it("command_prompt_sends_no_prompt", () => {
    const req = launchRequest({ ...choice("claude"), shell: "cmd" }, "fix it", programs, DOC, PROJECTS, SIZE);
    expect(req).toMatchObject({ agent: "claude", profile_id: "cmd" });
    expect(req).not.toHaveProperty("prompt");
    expect(nameSource({ ...choice("claude"), shell: "cmd" }, "fix it", programs, liveShells(PROFILES))).toBe("");
  });
});

describe("startingText", () => {
  it("names the agent by its CLI, and a shell by its profile name", () => {
    expect(startingText("claude", "my-app", liveAgents(null))).toBe("Starting Claude Code in my-app…");
    expect(startingText("Command Prompt", "my-app", [])).toBe("Starting Command Prompt in my-app…");
  });
});

describe("launchRequest", () => {
  const programs = liveAgents(null);
  it("agent request uses the chosen shell", () => {
    expect(launchRequest(choice("codex"), "fix the bug", programs, DOC, PROJECTS, SIZE)).toEqual({
      agent: "codex",
      profile_id: "pwsh",
      cwd: "C:/dev/my-app",
      prompt: "fix the bug",
      mode: "manual",
      cols: 120,
      rows: 30,
      elevated: false,
    });
  });
  it("carries the chosen mode for an agent, and none for a shell", () => {
    expect(launchRequest({ ...choice("claude"), mode: "plan" }, "", programs, DOC, PROJECTS, SIZE).mode).toBe("plan");
    expect("mode" in launchRequest({ ...choice("profile:pwsh"), mode: "plan" }, "", programs, DOC, PROJECTS, SIZE)).toBe(false);
  });
  it("leaves the prompt out when it is empty or blank", () => {
    expect(launchRequest(choice("claude"), "", programs, DOC, PROJECTS, SIZE)).not.toHaveProperty("prompt");
    expect(launchRequest(choice("claude"), "  \n", programs, DOC, PROJECTS, SIZE)).not.toHaveProperty("prompt");
  });
  it("shell request carries no prompt", () => {
    const req = launchRequest(choice("profile:cmd"), "dir", programs, DOC, PROJECTS, SIZE);
    expect(req).toEqual({ profile_id: "cmd", cwd: "C:/dev/my-app", cols: 120, rows: 30, elevated: false });
    expect(req).not.toHaveProperty("agent");
    expect(req).not.toHaveProperty("prompt");
  });
  it("leaves the folder out when none is chosen, so the core uses the home folder", () => {
    expect(launchRequest({ ...choice("claude"), projectId: "" }, "", programs, DOC, PROJECTS, SIZE)).not.toHaveProperty("cwd");
  });
});

describe("initialFocus", () => {
  const el = (name: string) => ({ name }) as unknown as HTMLElement;
  const root = (present: string[]) => ({ querySelector: (s: string) => (present.includes(s) ? el(s) : null) });
  it("is the prompt when there is one, so Enter starts", () => {
    expect(initialFocus(root([".launcher__prompt", ".launcher__start"]))).toEqual(el(".launcher__prompt"));
  });
  it("is the Start button for a shell profile, which has no prompt, so Enter still starts (not the program chip, which opens a list)", () => {
    expect(initialFocus(root([".launcher__chip", ".launcher__start"]))).toEqual(el(".launcher__start"));
  });
});

describe("refocus", () => {
  const at = (picks: number, prompt: boolean, ready: boolean) => ({ picks, prompt, ready });
  it("does nothing on the first look or when neither a pick nor readiness changed", () => {
    expect(refocus(null, at(0, true, false))).toBeNull();
    expect(refocus(at(1, true, true), at(1, true, true))).toBeNull();
  });
  it("moves to Start when a pick leaves no prompt, so Enter starts it (not the chip, where Enter reopens the list)", () => {
    expect(refocus(at(0, true, true), at(1, false, true))).toBe("start");
    expect(refocus(at(1, false, true), at(2, false, true))).toBe("start");
  });
  it("waits for Start to be enabled, then moves to it", () => {
    expect(refocus(at(0, false, false), at(0, false, false))).toBeNull();
    expect(refocus(at(0, false, false), at(0, false, true))).toBe("start");
  });
  it("moves to the prompt when an agent is picked", () => {
    expect(refocus(at(0, false, true), at(1, true, true))).toBe("prompt");
    expect(refocus(at(1, true, true), at(2, true, true))).toBe("prompt");
  });
  it("leaves the prompt alone when only readiness changed", () => {
    expect(refocus(at(0, true, false), at(0, true, true))).toBeNull();
  });
});

describe("focusStateOf", () => {
  const claude = LAUNCH_AGENTS.find((p) => p.id === "claude")!;
  it("a shell's late answer that drops the agent moves no focus: the person may be typing in the prompt or the Name field", () => {
    // Claude while Ubuntu is asked, then Ubuntu has none: the prompt goes, but nobody picked anything.
    const asking = focusStateOf({ picks: 1, program: claude, shellFamily: "wsl", loaded: true });
    const answered = focusStateOf({ picks: 1, program: undefined, shellFamily: "wsl", loaded: true });
    expect(answered.prompt).toBe(false);
    expect(refocus(asking, answered)).toBeNull();
  });
  it("has a prompt only for an agent that takes one, outside Command Prompt", () => {
    expect(focusStateOf({ picks: 0, program: claude, shellFamily: "pwsh", loaded: false })).toEqual({ picks: 0, prompt: true, ready: false });
    expect(focusStateOf({ picks: 0, program: claude, shellFamily: "cmd", loaded: true }).prompt).toBe(false);
    expect(focusStateOf({ picks: 0, program: { ...claude, noFirst: true }, shellFamily: "pwsh", loaded: true }).prompt).toBe(false);
  });
});

describe("lostFocus", () => {
  const el = (name: string) => ({ name }) as unknown as HTMLElement;
  const root = (present: string[]) => ({ querySelector: (s: string) => (present.includes(s) ? el(s) : null) });
  it("is the prompt when there is one, else the agent chip, which says why the agent went (never Start, where Enter starts a bare shell)", () => {
    expect(lostFocus(root([".launcher__prompt", '.launcher__chip[aria-label^="Agent:"]', ".launcher__start"]))).toEqual(el(".launcher__prompt"));
    expect(lostFocus(root(['.launcher__chip[aria-label^="Agent:"]', ".launcher__start"]))).toEqual(el('.launcher__chip[aria-label^="Agent:"]'));
  });
});

describe("startFailure", () => {
  it("shows the core's refusal word for word", () => {
    const text = "Command Prompt can't pass a prompt safely. Use PowerShell or start without a prompt.";
    expect(startFailure(new ApiError(400, text))).toBe(text);
  });
  it("says what failed for anything else", () => {
    expect(startFailure(new TypeError("Failed to fetch"))).toBe("Couldn't start the session: TypeError: Failed to fetch");
  });
});

describe("toLauncherProject", () => {
  it("uses the path as the id and drops a missing branch", () => {
    expect(toLauncherProject({ path: "C:/a", name: "a", branch: null, last_used_ms: 5 })).toEqual({ id: "C:/a", name: "a", path: "C:/a", lastUsedMs: 5 });
    expect(toLauncherProject({ path: "C:/b", name: "b", branch: "main", last_used_ms: 6 }).branch).toBe("main");
  });
});


describe("preselectMode", () => {
  const docWith = (launcher: LayoutDoc["launcher"]): LayoutDoc => ({ tabs: [], view: {} as LayoutDoc["view"], launcher });
  it("is the mode last used in the folder, and nothing when none is remembered", () => {
    const doc = docWith({
      "C:/a": { profile_id: "pwsh", agent: "claude", seq: 1, mode: "plan" },
      "C:/b": { profile_id: "pwsh", agent: "claude", seq: 2, mode: null },
    });
    expect(preselectMode(doc, "C:/a")).toBe("plan");
    expect(preselectMode(doc, "C:/b")).toBeUndefined();
    expect(preselectMode(doc, "C:/never-used")).toBeUndefined();
    expect(preselectMode(null, "C:/a")).toBeUndefined();
  });
});

describe("starting a task in a worktree", () => {
  const programs = liveAgents(null);
  const task: Task = { id: "t1", name: "fix-login", branch: "marshell/fix-login", base_branch: "main", repo_root: "C:/dev/my-app", path: "C:/dev/.marshell/my-app/fix-login", created_ms: 1, folder_missing: false };
  const on = (program: ProgramId): LauncherChoice => ({ ...choice(program), worktree: true });
  const NO_GIT: Project[] = [{ id: "C:/dev/scratch", name: "scratch", path: "C:/dev/scratch", lastUsedMs: 1 }];

  it("goes through a confirm step for any program in a git folder with the worktree on", () => {
    expect(wantsTask(on("claude"), PROJECTS)).toBe(true);
    expect(wantsTask(choice("claude"), PROJECTS)).toBe(false);
    expect(wantsTask(on("profile:cmd"), PROJECTS)).toBe(true);
    expect(wantsTask(choice("profile:cmd"), PROJECTS)).toBe(false);
    expect(wantsTask({ ...on("claude"), projectId: "C:/dev/scratch" }, NO_GIT)).toBe(false);
    expect(wantsTask({ ...on("claude"), projectId: "" }, PROJECTS)).toBe(false);
  });

  it("names the task from the prompt, and from nothing for a shell, which has none (the core then picks task-<n>)", () => {
    expect(nameSource(on("claude"), "Fix login", programs)).toBe("Fix login");
    // Text typed for an agent before switching to a shell is not sent, so it doesn't name the task either.
    expect(nameSource(on("profile:cmd"), "Fix login", programs, liveShells(PROFILES))).toBe("");
  });

  it("asks the core to plan and create in the chosen folder with the name as typed", () => {
    expect(taskRequest(on("claude"), PROJECTS, "fix-login")).toEqual({ cwd: "C:/dev/my-app", name: "fix-login" });
  });

  it("creates with the commands the confirm view shows, and not before it shows them", () => {
    const plan = taskRequest(on("claude"), PROJECTS, "fix-login");
    const commands = ["git -C C:/dev/my-app worktree add -b marshell/fix-login C:/dev/.marshell/my-app/fix-login main"];
    expect(createRequest(plan, { state: "ready", commands })).toEqual({ cwd: "C:/dev/my-app", name: "fix-login", commands });
    expect(createRequest(plan, { state: "loading" })).toBeNull();
    expect(createRequest(plan, { state: "refused", reason: "No." })).toBeNull();
    expect(createRequest(plan, null)).toBeNull();
  });

  it("plans again when the core refused the create, so the next try shows what it would run now", () => {
    expect(replansAfter(new ApiError(409, "The task changed while you were reading the commands. Review them again."))).toBe(true);
    expect(replansAfter(new ApiError(500, "boom"))).toBe(false);
    expect(replansAfter(new Error("network"))).toBe(false);
  });

  it("launches in the task's folder, and only after the create resolved", async () => {
    const log: string[] = [];
    let release: (t: Task) => void = () => {};
    const create = vi.fn(() => new Promise<Task>((resolve) => (release = resolve)));
    const launch = vi.fn(async (t: Task) => {
      log.push(`launch ${launchRequest(on("claude"), "", programs, DOC, PROJECTS, SIZE, t.path).cwd}`);
      return "started";
    });
    const done = createThenLaunch(create, launch);
    await Promise.resolve();
    expect(create).toHaveBeenCalledTimes(1);
    expect(launch).not.toHaveBeenCalled();
    release(task);
    await expect(done).resolves.toBe("started");
    expect(log).toEqual(["launch C:/dev/.marshell/my-app/fix-login"]);
  });

  it("does not launch when the create fails, and passes the failure on", async () => {
    const launch = vi.fn();
    await expect(createThenLaunch(() => Promise.reject(new ApiError(409, "No free name.")), launch)).rejects.toMatchObject({ message: "No free name." });
    expect(launch).not.toHaveBeenCalled();
  });

  it("makes one worktree per request: a retry after a failed start reuses it", async () => {
    const memo = rememberTasks();
    const create = vi.fn(async () => task);
    const req = { cwd: "C:/dev/my-app", name: "fix-login", commands: ["git x"] };
    await memo.get(req, create);
    await memo.get({ ...req }, create);
    expect(create).toHaveBeenCalledTimes(1);
  });

  it("makes a new worktree for a different request", async () => {
    const memo = rememberTasks();
    const create = vi.fn(async () => task);
    await memo.get({ cwd: "C:/dev/my-app", name: "fix-login", commands: ["git x"] }, create);
    await memo.get({ cwd: "C:/dev/my-app", name: "fix-logout", commands: ["git x"] }, create);
    expect(create).toHaveBeenCalledTimes(2);
  });

  it("forgets the task on Back, so the next create runs the commands that were shown", async () => {
    const memo = rememberTasks();
    const create = vi.fn(async () => task);
    const req = { cwd: "C:/dev/my-app", name: "fix-login", commands: ["git x"] };
    await memo.get(req, create);
    memo.forget();
    await memo.get(req, create);
    expect(create).toHaveBeenCalledTimes(2);
  });

  it("does not remember a create that failed", async () => {
    const memo = rememberTasks();
    const create = vi.fn().mockRejectedValueOnce(new ApiError(409, "No free name.")).mockResolvedValueOnce(task);
    const req = { cwd: "C:/dev/my-app", name: "fix-login", commands: ["git x"] };
    await expect(memo.get(req, create)).rejects.toMatchObject({ message: "No free name." });
    await expect(memo.get(req, create)).resolves.toBe(task);
  });

  it("says what is happening: the worktree first, then the program", () => {
    expect(progressText("creating", "Claude")).toBe("Creating the worktree…");
    expect(progressText("starting", "Claude")).toBe("Starting Claude…");
  });

  it("launchRequest takes the task folder over the project's, and changes nothing without one", () => {
    expect(launchRequest(on("claude"), "go", programs, DOC, PROJECTS, SIZE, task.path)).toEqual({
      agent: "claude",
      profile_id: "pwsh",
      cwd: task.path,
      prompt: "go",
      mode: "manual",
      cols: 120,
      rows: 30,
      elevated: false,
    });
    // With the worktree flag on and no task, nothing about the request changes: the flag is never sent.
    expect(launchRequest(on("claude"), "go", programs, DOC, PROJECTS, SIZE)).toEqual(launchRequest(choice("claude"), "go", programs, DOC, PROJECTS, SIZE));
  });
});

describe("chooseFolder", () => {
  const recent: Project[] = [{ id: "c:\\dev\\app", name: "app", path: "c:\\dev\\app", lastUsedMs: 5 }];
  it("selects the recent project spelt differently (case, trailing separator) and adds nothing", () => {
    const folder: FolderInfo = { path: "C:\\Dev\\App\\", name: "App", branch: null };
    expect(chooseFolder(recent, folder)).toEqual({ projectId: "c:\\dev\\app", extra: null });
  });
  it("builds a project from an unknown folder, its path as the id", () => {
    const folder: FolderInfo = { path: "C:\\other\\thing", name: "thing", branch: "main" };
    expect(chooseFolder(recent, folder)).toEqual({
      projectId: "C:\\other\\thing",
      extra: { id: "C:\\other\\thing", name: "thing", path: "C:\\other\\thing", branch: "main", lastUsedMs: 0 },
    });
  });
  it("leaves the branch out when the folder has none", () => {
    const extra = chooseFolder([], { path: "C:\\x", name: "x", branch: null }).extra;
    expect(extra).toEqual({ id: "C:\\x", name: "x", path: "C:\\x", lastUsedMs: 0 });
    expect(extra && "branch" in extra).toBe(false);
  });
});

describe("samePath", () => {
  it("ignores case and a trailing separator on a Windows path, not on a Unix one", () => {
    expect(samePath("C:\\Dev\\App\\", "c:\\dev\\app")).toBe(true);
    expect(samePath("/Dev/App", "/dev/app")).toBe(false);
  });
});

describe("checkState", () => {
  const folder: FolderInfo = { path: "C:\\a", name: "a", branch: null };
  it("a found folder is found", () => {
    expect(checkState("c:/a", { ok: folder })).toEqual({ path: "c:/a", state: "found", folder });
  });
  it("a 400 (not a full path) shows no row", () => {
    expect(checkState("/x", { status: 400 })).toBeNull();
  });
  it("only a 404 is missing; any other failure is failed", () => {
    expect(checkState("C:\\nope", { status: 404 })).toEqual({ path: "C:\\nope", state: "missing" });
    expect(checkState("C:\\nope", { status: 500 })).toEqual({ path: "C:\\nope", state: "failed" });
    expect(checkState("C:\\nope", { status: 0 })).toEqual({ path: "C:\\nope", state: "failed" });
  });
});

describe("latestOnly", () => {
  it("only the last number taken is the latest", () => {
    const seq = latestOnly();
    const a = seq.next();
    const b = seq.next();
    expect(seq.isLatest(a)).toBe(false);
    expect(seq.isLatest(b)).toBe(true);
  });
});

describe("browseFolder", () => {
  const folder: FolderInfo = { path: "C:\dev", name: "dev", branch: null };
  const setup = (over: Partial<Parameters<typeof browseFolder>[0]> = {}) => {
    const take = vi.fn();
    const fail = vi.fn();
    const focusField = vi.fn();
    const deps = { start: "C:\a", pick: vi.fn().mockResolvedValue("C:\dev"), check: vi.fn().mockResolvedValue(folder), take, fail, focusField, ...over };
    return { deps, take, fail, focusField };
  };
  it("passes the start folder to the picker and takes the checked folder without moving focus", async () => {
    const { deps, take, fail, focusField } = setup();
    await browseFolder(deps);
    expect(deps.pick).toHaveBeenCalledWith("C:\a");
    expect(deps.check).toHaveBeenCalledWith("C:\dev");
    expect(take).toHaveBeenCalledWith(folder);
    expect(fail).not.toHaveBeenCalled();
    expect(focusField).not.toHaveBeenCalled();
  });
  it("refocuses the field when the picker is cancelled", async () => {
    const { deps, take, fail, focusField } = setup({ pick: vi.fn().mockResolvedValue(null) });
    await browseFolder(deps);
    expect(focusField).toHaveBeenCalledTimes(1);
    expect(take).not.toHaveBeenCalled();
    expect(fail).not.toHaveBeenCalled();
  });
  it("reports a picker that throws and refocuses the field", async () => {
    const { deps, take, fail, focusField } = setup({ pick: vi.fn().mockRejectedValue(new Error("no dialog")) });
    await browseFolder(deps);
    expect(fail).toHaveBeenCalledWith("Couldn't open the folder picker: no dialog");
    expect(focusField).toHaveBeenCalledTimes(1);
    expect(take).not.toHaveBeenCalled();
  });
  it("reports a picked folder the core can't use and refocuses the field", async () => {
    const { deps, take, fail, focusField } = setup({ check: vi.fn().mockRejectedValue(new Error("gone")) });
    await browseFolder(deps);
    expect(fail).toHaveBeenCalledWith("Couldn't use C:\dev: gone");
    expect(focusField).toHaveBeenCalledTimes(1);
    expect(take).not.toHaveBeenCalled();
  });
});

describe("an administrator start", () => {
  it("the_admin_chip_toggles_with_alt_a_and_sends_elevated: the request carries elevated, and an agent asks for Manual", () => {
    const programs = liveAgents(CATALOG);
    expect(launchRequest(choice("claude"), "", programs, DOC, PROJECTS, SIZE).elevated).toBe(false);
    const req = launchRequest({ ...choice("claude"), mode: "bypass" }, "", programs, DOC, PROJECTS, SIZE, undefined, true);
    expect(req.elevated).toBe(true);
    expect(req.mode).toBe("manual");
    expect(launchRequest(choice("profile:pwsh"), "", programs, DOC, PROJECTS, SIZE, undefined, true)).toMatchObject({ elevated: true, profile_id: "pwsh" });
  });
});

describe("the name in the request", () => {
  const programs = liveAgents(CATALOG);
  it("the_name_sent_is_trimmed_and_empty_means_the_default", () => {
    const send = (name?: string) => launchRequest(choice("claude"), "", programs, DOC, PROJECTS, SIZE, undefined, false, name);
    expect(send()).not.toHaveProperty("name");
    expect(send("  ")).not.toHaveProperty("name");
    expect(send("\t\n")).not.toHaveProperty("name");
    expect(send(" fix ").name).toBe("fix");
    expect(send("a\tb\nc").name).toBe("abc");
    expect(launchRequest({ ...choice("claude"), agent: null }, "", programs, DOC, PROJECTS, SIZE, undefined, false, " sh ").name).toBe("sh");
  });
});
