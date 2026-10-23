import { isAgentId } from "../../components/AgentMark/agents";
import type { SessionKind } from "../../components/AgentMark/kinds";
import { toolForProgram } from "../../components/AgentMark/tools";
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

/**
 * The sidebar row for one open session the core lists. An agent with live status shows it. One without shows the pty
 * fallback (Working or Idle, noted as limited) or, with no activity either, a limited row that says why. A shell row's
 * status comes from its prompt marks: "working" while a command runs, and "idle" at the prompt (or before any mark),
 * naming a failed last command. `now` is the clock the ages count from.
 */
export function toRow(info: SessionInfo, profiles: Profile[], setup?: { claude?: HookInstall }, now: number = Date.now()): RowModel {
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
          ? info.agent
          : "generic";
  const base = {
    id: info.tab_id,
    number: info.number,
    name: info.name,
    project: lastSegment(info.project),
    projectKey: info.project,
    agent,
    ...(info.mode === null ? {} : { mode: info.mode }),
    ...(info.muted ? { muted: true } : {}),
  };
  if (info.state.kind === "ended") return { ...base, status: "ended", phrase: ENDED_PHRASE };
  if (info.state.kind === "exited") return { ...base, status: "ended", phrase: `Exited with code ${info.state.code}` };
  if (info.paused) return { ...base, status: "paused", phrase: "" };
  if (info.live === null && info.attention !== null) return { ...base, status: "needs-question", phrase: info.attention.message ?? "Wants your attention" };
  if (info.agent !== null) {
    if (info.live !== null) return { ...base, ...liveRow(info.live, info.stuck, now) };
    // Claude Code without Marshell's hooks: say so and offer the setup, ahead of the output fallback. An unknown state (not fetched yet) says and offers nothing.
    if (info.agent === "claude" && setup?.claude !== undefined && setup.claude !== "installed") return { ...base, status: "limited", phrase: "Status limited · set up", setupLink: true };
    const note = info.status_note === null ? LIMITED_NOTE : `${LIMITED_NOTE} · ${info.status_note}`;
    const statusNote = info.status_note === null ? {} : { statusNote: info.status_note };
    if (info.activity === "working") return { ...base, status: "working", phrase: `Working · ${LIMITED_NOTE}`, note, ...statusNote };
    if (info.activity === "idle") return { ...base, status: "idle", phrase: `Idle · ${LIMITED_NOTE}`, note, ...statusNote };
    return { ...base, status: "limited", phrase: info.status_note ?? "Status arrives with hooks" };
  }
  if (fg !== null) {
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
