import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import { toRow } from "./toRow";

const profile = (id: string, family: Profile["family"]): Profile => ({ id, name: id, command: [id], family, cwd: null, env: [], shell_integration: true, detected: true });
const PROFILES: Profile[] = [
  profile("pwsh", "power_shell"),
  profile("cmd", "cmd"),
  profile("git-bash", "posix"),
  profile("zsh", "posix"),
  profile("wsl-ubuntu", "wsl"),
];
const info = (patch: Partial<SessionInfo> = {}): SessionInfo => ({
  tab_id: "t1",
  number: 2,
  name: "Shell",
  agent: null,
  profile_id: "pwsh",
  cwd: "C:\\dev\\my-app",
  project: "C:\\dev\\my-app",
  mode: null,
  state: { kind: "running" },
  command: { kind: "unknown" },
  ...patch,
});

describe("toRow", () => {
  it("names the project after the repository, not the sub folder", () => {
    const row = toRow(info({ cwd: "C:\\w\\app\\src", project: "C:\\w\\app" }), PROFILES);
    expect(row.project).toBe("app");
    expect(row.projectKey).toBe("C:\\w\\app");
  });

  it("maps exited to ended with code phrase", () => {
    const row = toRow(info({ state: { kind: "exited", code: 3 } }), PROFILES);
    expect(row.status).toBe("ended");
    expect(row.phrase).toBe("Exited with code 3");
  });

  it("an ended tab reads Ended", () => {
    const row = toRow(info({ state: { kind: "ended" } }), PROFILES);
    expect(row.status).toBe("ended");
    expect(row.phrase).toContain("Ended");
  });

  it("agent session is limited, never working", () => {
    const row = toRow(info({ agent: "claude", name: "Claude Code" }), PROFILES);
    expect(row.agent).toBe("claude");
    expect(row.status).toBe("limited");
    expect(row.phrase).toBe("Status arrives with hooks");
  });

  it("an exited agent session is ended, not limited", () => {
    expect(toRow(info({ agent: "codex", state: { kind: "exited", code: 0 } }), PROFILES).status).toBe("ended");
  });

  it("a running shell is idle with no phrase", () => {
    const row = toRow(info(), PROFILES);
    expect(row.status).toBe("idle");
    expect(row.phrase).toBe("");
  });

  it("git bash profile maps to gitbash mark", () => {
    expect(toRow(info({ profile_id: "git-bash" }), PROFILES).agent).toBe("gitbash");
  });

  it("maps each shell family to its mark", () => {
    const kind = (profile_id: string) => toRow(info({ profile_id }), PROFILES).agent;
    expect(kind("pwsh")).toBe("powershell");
    expect(kind("cmd")).toBe("cmd");
    expect(kind("zsh")).toBe("bash");
    expect(kind("wsl-ubuntu")).toBe("wsl");
  });

  it("falls back to the generic program mark when the profile is gone", () => {
    expect(toRow(info({ profile_id: "deleted" }), PROFILES).agent).toBe("program");
  });

  it("carries id, number, name and the last folder of the cwd", () => {
    const row = toRow(info({ tab_id: "abc", number: 4, name: "api", cwd: "C:\\dev\\my-app\\" }), PROFILES);
    expect(row).toMatchObject({ id: "abc", number: 4, name: "api", project: "my-app" });
    expect(toRow(info({ project: "/home/me/proj" }), PROFILES).project).toBe("proj");
  });

  describe("shell command state", () => {
    const row = (command: SessionInfo["command"]) => toRow(info({ command }), PROFILES);

    it("before any mark the shell is idle with no phrase", () => {
      expect(row({ kind: "unknown" })).toMatchObject({ status: "idle", phrase: "" });
    });

    it("a running command is working", () => {
      expect(row({ kind: "running" })).toMatchObject({ status: "working", phrase: "Running a command" });
    });

    it("a failed last command is idle with the exit code", () => {
      expect(row({ kind: "prompt", last_exit: 2 })).toMatchObject({ status: "idle", phrase: "Last command failed (exit 2)" });
    });

    it("a prompt after success or with no code is idle with no phrase", () => {
      expect(row({ kind: "prompt", last_exit: 0 })).toMatchObject({ status: "idle", phrase: "" });
      expect(row({ kind: "prompt", last_exit: null })).toMatchObject({ status: "idle", phrase: "" });
    });

    it("an exited shell is ended whatever its command state", () => {
      const ended = toRow(info({ state: { kind: "exited", code: 0 }, command: { kind: "running" } }), PROFILES);
      expect(ended).toMatchObject({ status: "ended", phrase: "Exited with code 0" });
    });

    it("an agent session ignores command state", () => {
      expect(toRow(info({ agent: "claude", command: { kind: "running" } }), PROFILES).status).toBe("limited");
    });
  });
});
