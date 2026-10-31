import { isAgentId } from "../../components/AgentMark/agents";
import type { SessionKind } from "../../components/AgentMark/kinds";
import { toolForProgram } from "../../components/AgentMark/tools";
import type { HookAgent } from "../../lib/api/agents";
import type { HookInstall } from "../../generated/HookInstall";
import type { LiveState } from "../../generated/LiveState";
import type { LiveStatus } from "../../generated/LiveStatus";
import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";

/** The mark for a shell session, from its profile's family. A profile that no longer exists gets the generic program mark. */
export function shellKind(profile: Profile | undefined): SessionKind {
  switch (profile?.family) {
    case "power_shell":
      return "powershell";
    case "cmd":
      return "cmd";
    case "posix":
      return profile.id === "git-bash" ? "gitbash" : "bash";
    case "wsl":
      return "wsl";
    default:
      return "program";
  }
}

/** The last folder of a path, with either kind of separator and any trailing ones. */
export function lastSegment(path: string): string {
  return path.split(/[\\/]+/).filter(Boolean).pop() ?? path;
}

/** The row phrase of a tab with no process: it was open when the app last closed. The Resume strip tells it from an Exited tab by this. */
export const ENDED_PHRASE = "Ended last time";

/** The tooltip note of an agent row that has no live status but the output-activity fallback. */
export const LIMITED_NOTE = "Status limited";

/**
 * The glyph for a live state. A finished turn reads done-seen: the live app has no "seen" mechanism yet (nothing marks a row
 * as looked at), so a finished turn must not keep a row flagged as unseen for ever.
 */
const GLYPH: Record<LiveState, GlyphKind> = {
  idle: "idle",
  working: "working",
  needs_permission: "needs-permission",
  needs_question: "needs-question",
  done: "done-seen",
  error: "error",
  ended: "ended",
};

/** The row fields a live status gives. What the status line has not reported is left out, not zero. `now` is passed in so a row is a pure function of its input. */
function liveRow(live: LiveStatus, stuck: SessionInfo["stuck"], now: number): Pick<RowModel, "status" | "phrase" | "waitingMs" | "ageMs" | "model" | "effort" | "contextPct" | "subagents" | "recap"> {
  const since = Math.max(0, now - live.since_ms);
  const reason = live.state === "working" && stuck !== null ? stuck.reason === "repeat" ? "Same command 9× in 4 min" : "No progress for 10 min" : null;
  return {
    status: reason !== null ? "stuck" : GLYPH[live.state],
    phrase: reason ?? (live.state === "done" && live.recap !== null ? `${live.phrase} · ${live.recap.text}` : live.phrase),
    ...(live.state === "needs_permission" || live.state === "needs_question" ? { waitingMs: since } : {}),
    ...(live.state === "done" ? { ageMs: since } : {}),
    ...(live.model === null ? {} : { model: live.model }),
    ...(live.effort === null ? {} : { effort: live.effort }),
    ...(live.context_pct === null ? {} : { contextPct: live.context_pct }),
    ...(live.subagents === 0 ? {} : { subagents: live.subagents }),
    ...(live.recap === null ? {} : { recap: { text: live.recap.text, ageMs: Math.max(0, now - live.recap.at_ms) } }),
  };
}

/**
 * How often the rows' ages need recounting: every second while a session waits for you or an age shown is under a minute
 * (it shows seconds), every half minute when the ages shown are in minutes, and never when no row shows one.
 */
export function ageTick(infos: SessionInfo[], now: number): number | undefined {
  let tick: number | undefined;
  for (const { live } of infos) {
    if (live === null) continue;
    if (live.state === "needs_permission" || live.state === "needs_question") return 1000;
    const ages = [...(live.state === "done" ? [live.since_ms] : []), ...(live.recap === null ? [] : [live.recap.at_ms])];
    for (const at of ages) {
      if (now - at < 60_000) return 1000;
      tick = 30_000;
    }
  }
  return tick;
}

/**
 * The plan ring: the highest 5-hour use any Claude session reports for a window that has not reset, or nothing when none does.
 * A claude run inside a shell tab feeds that shell's row, not the ring. `until` is when the shown use's window resets.
 */
export function planRing(infos: SessionInfo[], now = Date.now()): { pct: number; until?: number } | undefined {
  let best: { pct: number; until?: number } | undefined;
  for (const i of infos) {
    if (i.agent !== "claude" || !i.live) continue;
    const p = i.live.five_hour_pct;
    if (p === null || !Number.isFinite(p)) continue;
    // A use from a window that has since reset says nothing about the new one: within a window, use only rises.
    const until = i.live.five_hour_resets_at === null ? undefined : i.live.five_hour_resets_at * 1000;
    if (until !== undefined && until <= now) continue;
    if (best === undefined || p > best.pct) best = until === undefined ? { pct: p } : { pct: p, until };
  }
  return best;
}

const isHookAgent = (agent: string): agent is HookAgent => agent === "claude" || agent === "codex" || agent === "antigravity";

/** The agents with a Rust adapter. Any other agent is a manifest: Marshell writes no hooks for it, so there are none to wait for. */
const isManifestAgent = (agent: string): boolean => agent !== "claude" && agent !== "codex" && agent !== "antigravity";

/** The pty fallback of an agent without live status: Working or Idle from its output, noted as limited. Undefined without activity. */
function fallbackRow(info: SessionInfo, base: Omit<RowModel, "status" | "phrase">): RowModel | undefined {
  const note = info.status_note === null ? LIMITED_NOTE : `${LIMITED_NOTE} · ${info.status_note}`;
  const statusNote = info.status_note === null ? {} : { statusNote: info.status_note };
  if (info.activity === "working") return { ...base, status: "working", phrase: `Working · ${LIMITED_NOTE}`, note, ...statusNote };
  if (info.activity === "idle") return { ...base, status: "idle", phrase: `Idle · ${LIMITED_NOTE}`, note, ...statusNote };
  return undefined;
}

/**
 * The sidebar row for one open session the core lists. An agent with live status shows it. One without shows the pty
 * fallback (Working or Idle, noted as limited) or, with no activity either, a limited row that says why. A shell row's
 * status comes from its prompt marks: "working" while a command runs, and "idle" at the prompt (or before any mark),
 * naming a failed last command. `now` is the clock the ages count from.
 */
export function toRow(info: SessionInfo, profiles: Profile[], setup?: Partial<Record<HookAgent, HookInstall>>, now: number = Date.now()): RowModel {
  const row = rowOf(info, profiles, setup, now);
  const outcome = info.setup?.outcome;
  if (info.setup === undefined || outcome === undefined || outcome.kind === "passed") return row;
  const text = outcome.kind === "failed" ? `Setup failed (exit ${outcome.code}).` : "Setup was closed before it finished.";
  return { ...row, setupNote: { text, tabId: info.setup.tab_id } };
}

/** Leaves the link out of a setup note whose tab is not in the session list any more. */
export function dropGoneSetupTabs(rows: RowModel[], infos: SessionInfo[]): RowModel[] {
  return rows.map((r) => {
    const tabId = r.setupNote?.tabId;
    if (r.setupNote === undefined || tabId === undefined || infos.some((i) => i.tab_id === tabId)) return r;
    return { ...r, setupNote: { text: r.setupNote.text } };
  });
}

function rowOf(info: SessionInfo, profiles: Profile[], setup: Partial<Record<HookAgent, HookInstall>> | undefined, now: number): RowModel {
  const profile = profiles.find((p) => p.id === info.profile_id);
  // The foreground only speaks for a shell that is running and not paused; a launched agent's row is its own.
  const fg = info.agent === null && info.state.kind === "running" && !info.paused ? info.foreground : null;
  const agent: SessionKind =
    fg !== null
      ? fg.agent !== null && isAgentId(fg.agent)
        ? fg.agent
        : toolForProgram(fg.program)
      : info.agent === null
        ? shellKind(profile)
        : isAgentId(info.agent)
          ? // A preset has its own mark ("deepseek" runs Claude Code); `info.agent` stays what the hooks and the status are keyed by.
            info.preset != null && isAgentId(info.preset)
            ? info.preset
            : info.agent
          : "generic";
  const base = {
    id: info.tab_id,
    number: info.number,
    name: info.name,
    project: lastSegment(info.project),
    projectKey: info.project,
    agent,
    // A manifest agent with the generic mark: until the catalog names it (nameAgents), its program's name is the best there is.
    ...(agent === "generic" && info.agent !== null && info.foreground !== null ? { agentName: info.foreground.program } : {}),
    ...(info.mode === null ? {} : { mode: info.mode }),
    ...(info.muted ? { muted: true } : {}),
    ...(info.ports !== undefined && info.ports.length > 0 ? { ports: info.ports } : {}),
    ...(info.references ? { references: info.references } : {}),
  };
  if (info.state.kind === "ended") return { ...base, status: "ended", phrase: ENDED_PHRASE };
  if (info.state.kind === "exited") return { ...base, status: "ended", phrase: `Exited with code ${info.state.code}` };
  if (info.paused) return { ...base, status: "paused", phrase: "" };
  // A limited Needs you shows over live status too: on a live row only the core's "the agent shows its own prompt" sets
  // one (Antigravity's status line), and its hooks keep the row live meanwhile. A bell stays behind live status.
  if (info.attention !== null && (info.live === null || info.attention.limited === true)) {
    const message = info.attention.message ?? "Wants your attention";
    // A prompt matched from a tool's output text is a guess, not a hook: say so.
    if (info.attention.limited === true) return { ...base, status: "needs-question", phrase: `${message} · ${LIMITED_NOTE}`, note: LIMITED_NOTE };
    return { ...base, status: "needs-question", phrase: message };
  }
  if (info.agent !== null) {
    if (info.live !== null) return { ...base, ...liveRow(info.live, info.stuck, now) };
    // A hook agent (Claude Code, Codex, Antigravity) without Marshell's hooks: say so and offer the setup, ahead of the output fallback. An unknown state (not fetched yet) says and offers nothing.
    // Installed hooks offer nothing, even for a Codex that has not trusted them yet: the row keeps the limited fallback and never claims they work.
    const hooks = isHookAgent(info.agent) ? setup?.[info.agent] : undefined;
    if (hooks !== undefined && hooks !== "installed") return { ...base, status: "limited", phrase: "Status limited · set up", setupLink: true };
    const fallback = fallbackRow(info, base);
    if (fallback !== undefined) return fallback;
    // A manifest agent has no hooks coming, so it never says it waits for them.
    return { ...base, status: "limited", phrase: info.status_note ?? (isManifestAgent(info.agent) ? LIMITED_NOTE : "Status arrives with hooks") };
  }
  if (fg !== null) {
    if (fg.agent !== null && isManifestAgent(fg.agent)) return fallbackRow(info, base) ?? { ...base, status: "limited", phrase: LIMITED_NOTE };
    if (fg.agent !== null) return { ...base, status: "limited", phrase: "Status arrives with hooks" };
    return { ...base, status: "working", phrase: `Running ${fg.command_line ?? fg.program}` };
  }
  const command = info.command;
  if (command.kind === "running") return { ...base, status: "working", phrase: "Running a command" };
  if (command.kind === "prompt" && command.last_exit !== null && command.last_exit !== 0) {
    return { ...base, status: "idle", phrase: `Last command failed (exit ${command.last_exit})` };
  }
  return { ...base, status: "idle", phrase: "" };
}

/**
 * Gives each generic-marked agent row (Crush, Aider, an agent of the person's own) its catalog name, so the mark and the row say
 * "Crush" and not "Custom agent". Without a catalog entry the row keeps the foreground program's name, and without that, no name.
 */
export function nameAgents(rows: RowModel[], infos: SessionInfo[], agents: { id: string; name: string }[] | undefined): RowModel[] {
  if (agents === undefined) return rows;
  return rows.map((r) => {
    if (r.agent !== "generic") return r;
    const id = infos.find((i) => i.tab_id === r.id)?.agent;
    const name = agents.find((a) => a.id === id)?.name;
    return name === undefined ? r : { ...r, agentName: name };
  });
}
