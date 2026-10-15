import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import type { Project } from "../launcher/filter";
import { startingText, type LauncherChoice } from "../launcher/launchChoice";
import { ApiError } from "../../lib/api/client";
import { launchRequest, livePrograms, startFailure, toLauncherProject } from "./launchPrograms";

const profile = (id: string, family: Profile["family"], name = id): Profile => ({ id, name, command: [id], family, cwd: null, env: [], detected: true });
const PROFILES: Profile[] = [profile("pwsh", "power_shell", "PowerShell 7"), profile("cmd", "cmd", "Command Prompt"), profile("git-bash", "posix", "Git Bash"), profile("wsl-ubuntu", "wsl", "Ubuntu")];
const DOC: ProfilesDoc = { profiles: PROFILES, default_id: "pwsh" };
const PROJECTS: Project[] = [{ id: "C:/dev/my-app", name: "my-app", branch: "main", path: "C:/dev/my-app", lastUsedMs: 1 }];
const SIZE = { cols: 120, rows: 30 };
const choice = (program: string): LauncherChoice => ({ program, projectId: "C:/dev/my-app", mode: "Manual", pace: "Balanced", worktree: false });

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
      cols: 120,
      rows: 30,
    });
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
