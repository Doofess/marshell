import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import { rowLabel } from "../sidebar/labels";
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
  paused: false,
  foreground: null,
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

  it("a running, paused agent is Paused, with no phrase", () => {
    const row = toRow(info({ agent: "claude", paused: true }), PROFILES);
    expect(row.status).toBe("paused");
    expect(row.phrase).toBe("");
    expect(rowLabel(row)).toContain("Paused");
  });

  it("a paused shell is Paused too, ahead of running a command", () => {
    expect(toRow(info({ paused: true, command: { kind: "running" } as SessionInfo["command"] }), PROFILES).status).toBe("paused");
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

  it("carries the mode the session started in, and none when the core has none", () => {
    expect(toRow(info({ agent: "claude", mode: "bypass" }), PROFILES).mode).toBe("bypass");
    expect(toRow(info({ agent: "claude", mode: null }), PROFILES).mode).toBeUndefined();
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

  describe("foreground", () => {
    const fg = (program: string, command_line: string | null, agent: string | null = null) => ({ program, command_line, agent });

    it("a shell running a program shows that program's mark and the command line", () => {
      const row = toRow(info({ foreground: fg("docker", "docker compose up") }), PROFILES);
      expect(row).toMatchObject({ agent: "docker", status: "working", phrase: "Running docker compose up" });
    });

    it("without a command line the phrase names the program, and an unknown program gets the neutral mark", () => {
      const row = toRow(info({ foreground: fg("fake-agent", null) }), PROFILES);
      expect(row).toMatchObject({ agent: "program", status: "working", phrase: "Running fake-agent" });
    });

    it("a known agent in a shell shows as that agent, limited until the hooks arrive", () => {
      const row = toRow(info({ foreground: fg("codex", "codex", "codex") }), PROFILES);
      expect(row).toMatchObject({ agent: "codex", status: "limited", phrase: "Status arrives with hooks" });
    });

    it("no foreground keeps the shell's own mark", () => {
      expect(toRow(info({ foreground: null }), PROFILES).agent).toBe("powershell");
    });

    it("paused, exited and ended win over the foreground", () => {
      const foreground = fg("docker", "docker compose up");
      expect(toRow(info({ foreground, paused: true }), PROFILES)).toMatchObject({ status: "paused", agent: "powershell" });
      expect(toRow(info({ foreground, state: { kind: "exited", code: 1 } }), PROFILES)).toMatchObject({ status: "ended", agent: "powershell" });
      expect(toRow(info({ foreground, state: { kind: "ended" } }), PROFILES)).toMatchObject({ status: "ended", agent: "powershell" });
    });

    it("a launched agent row ignores the foreground", () => {
      const row = toRow(info({ agent: "claude", foreground: fg("docker", "docker compose up") }), PROFILES);
      expect(row).toMatchObject({ agent: "claude", status: "limited" });
    });
  });

  describe("the set up link", () => {
    it("a claude row without hooks links to set up", () => {
      for (const claude of ["none", "partial", "orphaned"] as const) {
        const row = toRow(info({ agent: "claude" }), PROFILES, { claude });
        expect(row).toMatchObject({ status: "limited", phrase: "Status limited", setupLink: true });
      }
    });
    it("an installed claude row has no link", () => {
      const row = toRow(info({ agent: "claude" }), PROFILES, { claude: "installed" });
      expect(row.setupLink).toBeUndefined();
      expect(row.phrase).toBe("Status arrives with hooks");
    });
    it("a shell row never has one", () => {
      const row = toRow(info(), PROFILES, { claude: "none" });
      expect(row.setupLink).toBeUndefined();
      expect(row.phrase).not.toBe("Status limited");
      expect(toRow(info({ foreground: { program: "claude", command_line: null, agent: "claude" } }), PROFILES, { claude: "none" }).setupLink).toBeUndefined();
    });
    it("a codex row has none, and neither has a claude row before the state is known", () => {
      expect(toRow(info({ agent: "codex" }), PROFILES, { claude: "none" }).setupLink).toBeUndefined();
      expect(toRow(info({ agent: "claude" }), PROFILES).setupLink).toBeUndefined();
    });
  });
});
