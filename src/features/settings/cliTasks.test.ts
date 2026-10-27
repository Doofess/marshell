import { describe, expect, it } from "vitest";
import type { AgentInstall } from "../../generated/AgentInstall";
import type { SessionInfo } from "../../generated/SessionInfo";
import { cliTaskRequest, exitedTasks, runningCount, updateResult, type PendingTask } from "./cliTasks";

const info = (tab_id: string, over: Partial<SessionInfo> = {}) => ({ tab_id, agent: null, state: { kind: "running" }, foreground: null, ...over }) as unknown as SessionInfo;
const install = (version: string | null): AgentInstall => ({ agent: "claude", found: true, version, install: { kind: "unknown" }, update_command: null, newer_version: null });

describe("cliTaskRequest", () => {
  it("sets only the size and the task", () => {
    expect(cliTaskRequest("claude", "sign_in", { cols: 100, rows: 30 })).toStrictEqual({ cols: 100, rows: 30, cli_task: { agent: "claude", kind: "sign_in" } });
    expect(cliTaskRequest("codex", "update", { cols: 80, rows: 24 })).toStrictEqual({ cols: 80, rows: 24, cli_task: { agent: "codex", kind: "update" } });
  });
});

describe("runningCount", () => {
  it("counts running sessions of the agent, as a session or in the foreground", () => {
    const sessions = [
      info("a", { agent: "claude" }),
      info("b", { agent: "claude", state: { kind: "exited", code: 0 } }),
      info("c", { foreground: { program: "claude", command_line: null, agent: "claude" } }),
      info("d", { agent: "codex" }),
      info("e", { state: { kind: "ended" }, foreground: { program: "claude", command_line: null, agent: "claude" } }),
    ];
    expect(runningCount(sessions, "claude")).toBe(2);
    expect(runningCount(sessions, "codex")).toBe(1);
    expect(runningCount(sessions, "antigravity")).toBe(0);
  });
});

describe("exitedTasks", () => {
  const task = (tabId: string, kind: PendingTask["kind"] = "update"): PendingTask => ({ tabId, agent: "claude", kind });
  it("keeps a running tab and reports an exited, ended or removed one", () => {
    const prev = new Map([task("run"), task("exit", "sign_in"), task("end"), task("gone")].map((t) => [t.tabId, t]));
    const sessions = [info("run"), info("exit", { state: { kind: "exited", code: 1 } }), info("end", { state: { kind: "ended" } })];
    const { done, rest } = exitedTasks(prev, sessions);
    expect(done.map((t) => t.tabId)).toEqual(["exit", "end", "gone"]);
    expect([...rest.keys()]).toEqual(["run"]);
  });
});

describe("updateResult", () => {
  it("says the new version, the same version, or that it can't be read", () => {
    expect(updateResult("Claude Code", "1.0.0", install("1.1.0"))).toBe("Claude Code is now version 1.1.0.");
    expect(updateResult("Claude Code", "1.0.0", install("1.0.0"))).toBe("Claude Code is still version 1.0.0.");
    expect(updateResult("Claude Code", "1.0.0", install(null))).toBe("Couldn't read Claude Code's version.");
  });
});
