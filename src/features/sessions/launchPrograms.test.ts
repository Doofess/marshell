import { describe, expect, it, vi } from "vitest";
import type { Task } from "../../generated/Task";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { Project } from "../launcher/filter";
import { startingText, type LauncherChoice, type ProgramId } from "../launcher/launchChoice";
import { ApiError } from "../../lib/api/client";
import type { LayoutDoc } from "../../generated/LayoutDoc";
import type { FolderInfo } from "../../generated/FolderInfo";
import { browseFolder, checkState, chooseFolder, latestOnly, samePath, createRequest, createThenLaunch, initialFocus, launchRequest, nameSource, preselect, preselectMode, progressText, refocus, rememberTasks, livePrograms, replansAfter, startFailure, taskRequest, toLauncherProject, wantsTask } from "./launchPrograms";

const profile = (id: string, family: Profile["family"], name = id): Profile => ({ id, name, command: [id], family, cwd: null, env: [], shell_integration: true, detected: true });
const PROFILES: Profile[] = [profile("pwsh", "power_shell", "PowerShell 7"), profile("cmd", "cmd", "Command Prompt"), profile("git-bash", "posix", "Git Bash"), profile("wsl-ubuntu", "wsl", "Ubuntu")];
const DOC: ProfilesDoc = { profiles: PROFILES, default_id: "pwsh" };
const PROJECTS: Project[] = [{ id: "C:/dev/my-app", name: "my-app", branch: "main", path: "C:/dev/my-app", lastUsedMs: 1 }];
const SIZE = { cols: 120, rows: 30 };
const choice = (program: ProgramId): LauncherChoice => ({ program, projectId: "C:/dev/my-app", mode: "manual", pace: "Balanced", worktree: false });

describe("livePrograms", () => {
  it("agents come first and only the three", () => {
    const programs = livePrograms(PROFILES);
    expect(programs.slice(0, 3).map((p) => p.id)).toEqual(["claude", "codex", "antigravity"]);
    expect(programs.slice(0, 3).every((p) => p.group === "Agents")).toBe(true);
    expect(programs.slice(3).every((p) => p.group === "Shells")).toBe(true);
    expect(programs.length).toBe(3 + PROFILES.length);
  });
  it("lists one program per profile, named for it, with the mark of its family", () => {
    const shells = livePrograms(PROFILES).slice(3);
    expect(shells.map((p) => p.id)).toEqual(["profile:pwsh", "profile:cmd", "profile:git-bash", "profile:wsl-ubuntu"]);
    expect(shells.map((p) => p.kind)).toEqual(["powershell", "cmd", "gitbash", "wsl"]);
    expect(shells.map((p) => p.label)).toEqual(["PowerShell 7", "Command Prompt", "Git Bash", "Ubuntu"]);
  });
  it("lists just the agents when no profile loaded", () => {
    expect(livePrograms([]).map((p) => p.id)).toEqual(["claude", "codex", "antigravity"]);
  });
  it("starting text names a profile program by its profile name", () => {
    expect(startingText("profile:cmd", "my-app", livePrograms(PROFILES))).toBe("Starting Command Prompt in my-app…");
    expect(startingText("claude", "my-app", livePrograms(PROFILES))).toBe("Starting Claude Code in my-app…");
  });
});

describe("launchRequest", () => {
  const programs = livePrograms(PROFILES);
  it("agent request uses the default profile", () => {
    expect(launchRequest(choice("codex"), "fix the bug", programs, DOC, PROJECTS, SIZE)).toEqual({
      agent: "codex",
      profile_id: "pwsh",
      cwd: "C:/dev/my-app",
      prompt: "fix the bug",
      mode: "manual",
      cols: 120,
      rows: 30,
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
    expect(req).toEqual({ profile_id: "cmd", cwd: "C:/dev/my-app", cols: 120, rows: 30 });
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
  const at = (program: string, prompt: boolean, ready: boolean) => ({ program, prompt, ready });
  it("does nothing on the first look or when neither the program nor readiness changed", () => {
    expect(refocus(null, at("claude", true, false))).toBeNull();
    expect(refocus(at("claude", true, true), at("claude", true, true))).toBeNull();
  });
  it("moves to Start when a program without a prompt is picked, so Enter starts it (not the chip, where Enter reopens the list)", () => {
    expect(refocus(at("claude", true, true), at("profile:cmd", false, true))).toBe("start");
    expect(refocus(at("profile:cmd", false, true), at("profile:pwsh", false, true))).toBe("start");
  });
  it("waits for Start to be enabled, then moves to it", () => {
    expect(refocus(at("profile:cmd", false, false), at("profile:cmd", false, false))).toBeNull();
    expect(refocus(at("profile:cmd", false, false), at("profile:cmd", false, true))).toBe("start");
  });
  it("moves to the prompt when an agent is picked", () => {
    expect(refocus(at("profile:cmd", false, true), at("claude", true, true))).toBe("prompt");
    expect(refocus(at("claude", true, true), at("codex", true, true))).toBe("prompt");
  });
  it("leaves the prompt alone when only readiness changed", () => {
    expect(refocus(at("claude", true, false), at("claude", true, true))).toBeNull();
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

describe("preselect", () => {
  const programs = livePrograms(PROFILES);
  const docWith = (launcher: LayoutDoc["launcher"]): LayoutDoc => ({ tabs: [], view: {} as LayoutDoc["view"], launcher });
  it("remembered choice preselects only when its profile exists", () => {
    const doc = docWith({
      "C:/shell": { profile_id: "cmd", agent: null, seq: 1, mode: null },
      "C:/gone": { profile_id: "removed", agent: null, seq: 2, mode: null },
      "C:/agent": { profile_id: "pwsh", agent: "codex", seq: 3, mode: null },
      "C:/old-agent": { profile_id: "pwsh", agent: "aider", seq: 4, mode: null },
    });
    expect(preselect(doc, "C:/shell", programs)).toBe("profile:cmd");
    expect(preselect(doc, "C:/agent", programs)).toBe("codex");
    expect(preselect(doc, "C:/gone", programs)).toBeUndefined();
    expect(preselect(doc, "C:/old-agent", programs)).toBeUndefined();
    expect(preselect(doc, "C:/never-used", programs)).toBeUndefined();
    expect(preselect(null, "C:/shell", programs)).toBeUndefined();
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
  const programs = livePrograms(PROFILES);
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
    expect(nameSource(on("profile:cmd"), "Fix login", programs)).toBe("");
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
