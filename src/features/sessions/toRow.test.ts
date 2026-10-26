import { describe, expect, it } from "vitest";
import type { LiveStatus } from "../../generated/LiveStatus";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import { rowLabel, rowTitle } from "../sidebar/labels";
import { ageTick, nameAgents, planRing, toRow } from "./toRow";

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
  muted: false,
  attention: null,
  stuck: null,
  ...patch,
});

describe("toRow", () => {
  it("attention on a plain shell is needs-question with its message", () => {
    for (const patch of [{}, { command: { kind: "running" } }, { command: { kind: "prompt", last_exit: 2 } }, { foreground: { program: "docker", command_line: "docker build .", agent: null } }] satisfies Partial<SessionInfo>[]) {
      const row = toRow(info({ ...patch, attention: { message: "Build done", since_ms: 1000 } }), PROFILES);
      expect(row).toMatchObject({ status: "needs-question", phrase: "Build done" });
      expect(rowLabel(row)).toContain("Needs you");
      expect(rowLabel(row)).toContain("Build done");
    }
  });

  it("an_antigravity_row_with_a_limited_needs_you_says_status_limited", () => {
    const limited = toRow(info({ agent: "antigravity", attention: { message: "Waiting for you in the terminal", since_ms: 1000, limited: true } }), PROFILES);
    expect(limited).toMatchObject({ agent: "antigravity", status: "needs-question", phrase: "Waiting for you in the terminal · Status limited", note: "Status limited" });
    expect(rowLabel(limited)).toContain("Waiting for you in the terminal · Status limited");
  });
  it("an_antigravity_prompt_shows_over_its_live_status: agy's hooks keep the row live while its own prompt waits", () => {
    const live: LiveStatus = { state: "working", phrase: "Running `echo hi`", since_ms: 0, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: null, five_hour_resets_at: null, seven_day_pct: null, subagents: 0 };
    const row = toRow(info({ agent: "antigravity", live, attention: { message: "Waiting for you in the terminal", since_ms: 1000, limited: true } }), PROFILES);
    expect(row).toMatchObject({ status: "needs-question", phrase: "Waiting for you in the terminal · Status limited", note: "Status limited" });
    // A bell or a notification on a live row stays behind the live status, as before.
    const bell = toRow(info({ agent: "claude", live, attention: { message: null, since_ms: 1000 } }), PROFILES);
    expect(bell).toMatchObject({ status: "working" });
  });
  it("an_antigravity_row_shows_no_plan_meter: only Claude Code's five-hour use makes the ring", () => {
    const live: LiveStatus = { state: "idle", phrase: "", since_ms: 0, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: 80, five_hour_resets_at: null, seven_day_pct: null, subagents: 0 };
    expect(planRing([info({ agent: "antigravity", live })])).toBeUndefined();
    expect(toRow(info({ agent: "antigravity", live }), PROFILES)).not.toHaveProperty("planPct");
  });
  it("attention from a tool's prompt is marked Status limited, a plain one is not", () => {
    const limited = toRow(info({ attention: { message: "Asks to continue", since_ms: 1000, limited: true } }), PROFILES);
    expect(limited).toMatchObject({ status: "needs-question", phrase: "Asks to continue · Status limited", note: "Status limited" });
    expect(rowLabel(limited)).toContain("Asks to continue · Status limited");
    const plain = toRow(info({ attention: { message: "Asks to continue", since_ms: 1000 } }), PROFILES);
    expect(plain).toMatchObject({ phrase: "Asks to continue" });
    expect(plain.note).toBeUndefined();
  });

  it("attention without a message says Wants your attention", () => {
    expect(toRow(info({ attention: { message: null, since_ms: 1000 } }), PROFILES)).toMatchObject({ status: "needs-question", phrase: "Wants your attention" });
  });

  it("attention on a paused shell stays paused", () => {
    expect(toRow(info({ paused: true, attention: { message: "Build done", since_ms: 1000 } }), PROFILES)).toMatchObject({ status: "paused", phrase: "" });
  });

  it("attention on an agent without live status wins over setup and activity", () => {
    for (const agent of ["claude", "codex"]) {
      const row = toRow(info({ agent, activity: "working", attention: { message: "Build done", since_ms: 1000 } }), PROFILES, { claude: "none" });
      expect(row).toMatchObject({ status: "needs-question", phrase: "Build done" });
      expect(row.setupLink).toBeUndefined();
    }
  });

  it("attention leaves ended and exited rows unchanged", () => {
    const attention = { message: "Build done", since_ms: 1000 };
    expect(toRow(info({ attention, state: { kind: "ended" } }), PROFILES)).toMatchObject({ status: "ended", phrase: "Ended last time" });
    expect(toRow(info({ attention, state: { kind: "exited", code: 3 } }), PROFILES)).toMatchObject({ status: "ended", phrase: "Exited with code 3" });
  });

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

  describe("a manifest agent has no hooks to wait for", () => {
    const fg = (program: string, agent: string) => ({ program, command_line: program, agent });
    it("a row without activity reads Status limited, not Status arrives with hooks", () => {
      for (const agent of ["grok", "kimi", "aider"]) {
        expect(toRow(info({ agent }), PROFILES), agent).toMatchObject({ status: "limited", phrase: "Status limited" });
      }
    });
    it("a row with activity reads Working or Idle · Status limited", () => {
      expect(toRow(info({ agent: "grok", activity: "working" }), PROFILES)).toMatchObject({ status: "working", phrase: "Working · Status limited" });
      expect(toRow(info({ agent: "grok", activity: "idle" }), PROFILES)).toMatchObject({ status: "idle", phrase: "Idle · Status limited" });
    });
    it("keeps the core's own note when it gives one", () => {
      expect(toRow(info({ agent: "grok", status_note: "Grok 0.9 is older than Marshell knows" }), PROFILES).phrase).toBe("Grok 0.9 is older than Marshell knows");
    });
    it("a shell whose foreground is grok gets the same fallback", () => {
      expect(toRow(info({ foreground: fg("grok", "grok") }), PROFILES)).toMatchObject({ agent: "grok", status: "limited", phrase: "Status limited" });
      expect(toRow(info({ foreground: fg("grok", "grok"), activity: "working" }), PROFILES)).toMatchObject({ agent: "grok", status: "working", phrase: "Working · Status limited" });
      expect(toRow(info({ foreground: fg("grok", "grok"), activity: "idle" }), PROFILES)).toMatchObject({ status: "idle", phrase: "Idle · Status limited" });
    });
    it("leaves the copy of the adapters unchanged", () => {
      for (const agent of ["claude", "codex", "antigravity"]) {
        expect(toRow(info({ agent }), PROFILES).phrase, agent).toBe("Status arrives with hooks");
        expect(toRow(info({ foreground: fg(agent, agent), activity: "working" }), PROFILES), agent).toMatchObject({ status: "limited", phrase: "Status arrives with hooks" });
      }
    });
    it("a row started with the DeepSeek preset shows the deepseek mark and keeps Claude's hook copy", () => {
      const row = toRow(info({ agent: "claude", preset: "deepseek" }), PROFILES);
      expect(row).toMatchObject({ agent: "deepseek", phrase: "Status arrives with hooks" });
      expect(toRow(info({ agent: "claude" }), PROFILES).agent).toBe("claude");
      expect(toRow(info({ agent: "claude", preset: "unknown-preset" }), PROFILES).agent).toBe("claude");
    });
    it("a manifest agent with no mark of its own gets the generic mark", () => {
      expect(toRow(info({ agent: "aider" }), PROFILES).agent).toBe("generic");
    });
    it("names a generic-marked agent from the catalog, else from its program, else leaves the generic name", () => {
      const crush = info({ agent: "crush", foreground: { program: "crush-cli", command_line: null, agent: null } });
      const rows = [toRow(crush, PROFILES), toRow(info({ tab_id: "t2", agent: "own" }), PROFILES), toRow(info({ tab_id: "t3", agent: "claude" }), PROFILES)];
      expect(rows[0]!.agentName).toBe("crush-cli");
      const named = nameAgents(rows, [crush, info({ tab_id: "t2", agent: "own" }), info({ tab_id: "t3", agent: "claude" })], [{ id: "crush", name: "Crush" }]);
      expect(named[0]!.agentName).toBe("Crush");
      expect(named[1]!.agentName).toBeUndefined();
      expect(named[2]!.agentName).toBeUndefined();
      expect(rowLabel(named[0]!, "comfortable")).toContain("Crush");
      expect(rowLabel(named[0]!, "comfortable")).not.toContain("Custom agent");
      expect(rowLabel(named[1]!, "comfortable")).toContain("Custom agent");
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
    it("a claude row has none before the state is known, and a codex row none for Claude's state", () => {
      expect(toRow(info({ agent: "codex" }), PROFILES, { claude: "none" }).setupLink).toBeUndefined();
      expect(toRow(info({ agent: "claude" }), PROFILES).setupLink).toBeUndefined();
      expect(toRow(info({ agent: "claude" }), PROFILES, { codex: "none" }).setupLink).toBeUndefined();
    });
    it("a Codex row without hooks offers the setup", () => {
      for (const codex of ["none", "partial", "orphaned"] as const) {
        expect(toRow(info({ agent: "codex" }), PROFILES, { codex })).toMatchObject({ status: "limited", phrase: "Status limited · set up", setupLink: true });
      }
    });
    it("an installed but untrusted Codex keeps the limited fallback with no setup link", () => {
      const r = toRow(info({ agent: "codex", activity: "working" }), PROFILES, { codex: "installed" });
      expect(r.setupLink).toBeUndefined();
      expect(r).toMatchObject({ status: "working", phrase: "Working · Status limited" });
      expect(toRow(info({ agent: "codex" }), PROFILES, { codex: "installed" })).toMatchObject({ status: "limited", phrase: "Status arrives with hooks" });
    });
  });
});

describe("toRow: live status", () => {
  const NOW = 1_000_000;
  const live = (patch: Partial<LiveStatus> = {}): LiveStatus => ({ state: "idle", phrase: "", since_ms: NOW - 5000, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: null, five_hour_resets_at: null, seven_day_pct: null, subagents: 0, ...patch });
  const claude = (l: LiveStatus | null, patch: Partial<SessionInfo> = {}) => info({ agent: "claude", live: l, ...patch });
  const row = (l: LiveStatus | null, patch: Partial<SessionInfo> = {}) => toRow(claude(l, patch), PROFILES, { claude: "installed" }, NOW);

  it("a Codex row with live status shows its phrase", () => {
    const l = live({ state: "working", phrase: "Running a command" });
    expect(toRow(info({ agent: "codex", live: l }), PROFILES, { codex: "none" }, NOW)).toMatchObject({ agent: "codex", status: "working", phrase: "Running a command" });
  });

  it("attention is ignored when live is set", () => {
    for (const agent of [null, "claude"]) {
      const l = live({ state: "working", phrase: "Thinking" });
      expect(row(l, { agent, attention: { message: "Build done", since_ms: NOW } })).toEqual(row(l, { agent }));
    }
  });
  it("stuck repeat reads Same command 9× in 4 min", () => {
    const l = live({ state: "working", phrase: "Thinking", model: "Opus", effort: "high", context_pct: 42, subagents: 2, recap: { text: "Fixed it", at_ms: NOW - 7000 } });
    const r = row(l, { mode: "plan", stuck: { reason: "repeat", since_ms: NOW } });
    expect(r).toEqual({ ...row(l, { mode: "plan" }), status: "stuck", phrase: "Same command 9× in 4 min" });
    expect(rowLabel(r)).toContain("Same command 9× in 4 min");
    expect(rowTitle(r)).toContain("Same command 9× in 4 min");
  });
  it("stuck quiet reads No progress for 10 min", () => {
    const r = row(live({ state: "working", phrase: "Thinking" }), { stuck: { reason: "quiet", since_ms: NOW } });
    expect(r).toMatchObject({ status: "stuck", phrase: "No progress for 10 min" });
    expect(rowLabel(r)).toContain("No progress for 10 min");
    expect(rowTitle(r)).toContain("No progress for 10 min");
  });
  it("stuck outside Working is ignored", () => {
    for (const state of ["idle", "needs_permission", "needs_question", "done", "error", "ended"] as const) {
      const l = live({ state, phrase: "Original phrase" });
      expect(row(l, { stuck: { reason: "repeat", since_ms: NOW } })).toEqual(row(l));
    }
    expect(row(null, { stuck: { reason: "quiet", since_ms: NOW } })).toEqual(row(null));
  });

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
    info({ agent: "claude", live: { state, phrase: "", since_ms: NOW - since, recap: recapAt === null ? null : { text: "r", at_ms: NOW - recapAt }, model: null, effort: null, context_pct: null, five_hour_pct: null, five_hour_resets_at: null, seven_day_pct: null, subagents: 0 } });
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
    info({ tab_id: `t${n}`, live: { state: "idle", phrase: "", since_ms: 0, recap: null, model: null, effort: null, context_pct: null, five_hour_pct: pct, five_hour_resets_at: null, seven_day_pct: null, subagents: 0 } });
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
  it("leaves out a use whose 5-hour window has reset, and says when the shown one runs out", () => {
    const c = (n: number, pct: number, resets: number | null): SessionInfo => {
      const row = withPct(n, pct);
      return { ...row, agent: "claude", live: row.live && { ...row.live, five_hour_resets_at: resets } };
    };
    const now = 1_000_000_000_000;
    const gone = now / 1000 - 1;
    const soon = now / 1000 + 600;
    expect(planRing([c(1, 90, gone), c(2, 12, soon)], now)).toEqual({ pct: 12, until: soon * 1000 });
    expect(planRing([c(1, 90, gone)], now)).toBeUndefined();
    expect(planRing([c(1, 30, null)], now)).toEqual({ pct: 30 });
  });
});
