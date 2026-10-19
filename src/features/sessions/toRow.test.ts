import { describe, expect, it } from "vitest";
import type { LiveStatus } from "../../generated/LiveStatus";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import { rowLabel, rowTitle } from "../sidebar/labels";
import { ageTick, planRing, toRow } from "./toRow";

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
  live: null,
  activity: null,
  status_note: null,
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
        expect(row).toMatchObject({ status: "limited", phrase: "Status limited · set up", setupLink: true });
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

describe("toRow: live status", () => {
  const NOW = 1_000_000;
  const live = (patch: Partial<LiveStatus> = {}): LiveStatus => ({ state: "idle", phrase: "", since_ms: NOW - 5000, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: null, seven_day_pct: null, subagents: 0, ...patch });
  const claude = (l: LiveStatus | null, patch: Partial<SessionInfo> = {}) => info({ agent: "claude", live: l, ...patch });
  const row = (l: LiveStatus | null, patch: Partial<SessionInfo> = {}) => toRow(claude(l, patch), PROFILES, { claude: "installed" }, NOW);

  it.each([
    ["idle", "idle"],
    ["working", "working"],
    ["needs_permission", "needs-permission"],
    ["needs_question", "needs-question"],
    ["done", "done-seen"],
    ["error", "error"],
    ["ended", "ended"],
  ] as const)("maps %s to %s", (state, glyph) => {
    expect(row(live({ state, phrase: "p" })).status).toBe(glyph);
    expect(row(live({ state, phrase: "p" })).phrase).toBe("p");
  });
  it("carries model, effort, context and subagents, and leaves out what is null or 0", () => {
    const r = row(live({ model: "Opus", effort: "high", context_pct: 42, subagents: 2 }));
    expect(r).toMatchObject({ model: "Opus", effort: "high", contextPct: 42, subagents: 2 });
    const bare = row(live());
    expect(Object.keys(bare)).not.toEqual(expect.arrayContaining(["model"]));
    for (const k of ["model", "effort", "contextPct", "subagents", "recap"]) expect(Object.keys(bare)).not.toContain(k);
  });
  it("reads Done · recap on a done row that has one, and Done without", () => {
    expect(row(live({ state: "done", phrase: "Done", recap: { text: "Fixed it", at_ms: NOW - 7000 } })).phrase).toBe("Done · Fixed it");
    expect(row(live({ state: "done", phrase: "Done" })).phrase).toBe("Done");
    expect(row(live({ state: "idle", phrase: "Idle", recap: { text: "Fixed it", at_ms: NOW - 7000 } })).phrase).toBe("Idle");
  });
  it("gives the recap its age and a done row its age", () => {
    const r = row(live({ state: "done", since_ms: NOW - 3000, recap: { text: "Fixed it", at_ms: NOW - 7000 } }));
    expect(r.recap).toEqual({ text: "Fixed it", ageMs: 7000 });
    expect(r.ageMs).toBe(3000);
    expect(r.waitingMs).toBeUndefined();
  });
  it("gives a needs-you row how long it has waited", () => {
    expect(row(live({ state: "needs_permission", since_ms: NOW - 9000 })).waitingMs).toBe(9000);
    expect(row(live({ state: "needs_question", since_ms: NOW - 2000 })).waitingMs).toBe(2000);
  });
  it("never reports a negative age when the clock is behind", () => {
    expect(row(live({ state: "needs_permission", since_ms: NOW + 500 })).waitingMs).toBe(0);
  });
  it("falls back to output activity with a status-limited note on the tooltip", () => {
    const w = row(null, { activity: "working" });
    expect(w).toMatchObject({ status: "working", phrase: "Working · Status limited", note: "Status limited" });
    const i = row(null, { activity: "idle" });
    expect(i).toMatchObject({ status: "idle", phrase: "Idle · Status limited", note: "Status limited" });
    expect(rowTitle(w)).toContain("Status limited");
  });
  it("says set up for a Claude row without hooks, and the core's note otherwise", () => {
    const setup = toRow(claude(null, { status_note: "x" }), PROFILES, { claude: "none" }, NOW);
    expect(setup).toMatchObject({ status: "limited", phrase: "Status limited · set up", setupLink: true });
    const noted = row(null, { status_note: "Live status is off for Claude Code" });
    expect(noted).toMatchObject({ status: "limited", phrase: "Live status is off for Claude Code" });
    expect(noted.setupLink).toBeUndefined();
    expect(row(null).phrase).toBe("Status arrives with hooks");
  });
  it("keeps offering the setup for a Claude row without hooks, even while its output shows activity", () => {
    const r = toRow(claude(null, { activity: "working" }), PROFILES, { claude: "none" }, NOW);
    expect(r).toMatchObject({ status: "limited", phrase: "Status limited · set up", setupLink: true });
  });
  it("puts the core's reason in the fallback row's note", () => {
    const r = row(null, { activity: "idle", status_note: "Live status is off for Claude Code" });
    expect(r).toMatchObject({ status: "idle", phrase: "Idle · Status limited", note: "Status limited · Live status is off for Claude Code", statusNote: "Live status is off for Claude Code" });
  });
  it("puts the limited status in the fallback row's accessible name", () => {
    const r = row(null, { activity: "idle", status_note: "Live status is off for Claude Code" });
    expect(rowLabel(r)).toContain("Idle · Status limited. Claude, Idle. Status limited: Live status is off for Claude Code.");
    expect(rowLabel(row(null, { activity: "idle" }))).not.toContain("Status limited:");
  });
  it("lets live status win over a missing setup", () => {
    expect(toRow(claude(live({ state: "working", phrase: "Thinking" })), PROFILES, { claude: "none" }, NOW)).toMatchObject({ status: "working", phrase: "Thinking" });
  });
});

describe("ageTick", () => {
  const NOW = 1_000_000;
  const at = (state: LiveStatus["state"], since: number, recapAt: number | null = null): SessionInfo =>
    info({ agent: "claude", live: { state, phrase: "", since_ms: NOW - since, recap: recapAt === null ? null : { text: "r", at_ms: NOW - recapAt }, model: null, effort: null, context_pct: null, five_hour_pct: null, seven_day_pct: null, subagents: 0 } });
  it("counts every second while a session waits for you or an age is under a minute", () => {
    expect(ageTick([at("needs_permission", 600_000)], NOW)).toBe(1000);
    expect(ageTick([at("done", 5000)], NOW)).toBe(1000);
    expect(ageTick([at("working", 600_000, 30_000)], NOW)).toBe(1000);
  });
  it("counts every half minute when the ages shown are in minutes", () => {
    expect(ageTick([at("done", 600_000, 600_000)], NOW)).toBe(30_000);
  });
  it("does not tick when no row shows an age", () => {
    expect(ageTick([at("working", 5000), info()], NOW)).toBeUndefined();
    expect(ageTick([], NOW)).toBeUndefined();
  });
});

describe("planRing", () => {
  const withPct = (n: number, pct: number | null): SessionInfo =>
    info({ tab_id: `t${n}`, live: { state: "idle", phrase: "", since_ms: 0, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: pct, seven_day_pct: null, subagents: 0 } });
  it("takes the highest five-hour use among the sessions that report one", () => {
    const c = (n: number, pct: number | null) => ({ ...withPct(n, pct), agent: "claude" });
    expect(planRing([c(1, 20), c(2, 61), c(3, null), info()])).toEqual({ pct: 61 });
  });
  it("counts only Claude rows: a claude run inside a shell tab does not feed the ring", () => {
    const shell = withPct(1, 85);
    const claudeRow = { ...withPct(2, 4), agent: "claude" };
    expect(planRing([shell, claudeRow])).toEqual({ pct: 4 });
    expect(planRing([shell])).toBeUndefined();
  });
  it("is undefined when none does", () => {
    expect(planRing([{ ...withPct(1, null), agent: "claude" }, info()])).toBeUndefined();
    expect(planRing([])).toBeUndefined();
  });
});
