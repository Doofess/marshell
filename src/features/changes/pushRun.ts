import type { PushPlan } from "../../generated/PushPlan";
import type { PushProgress } from "../../generated/PushProgress";

/** Where a Create pull request or Push sheet is. The sheets draw from this; the calls and the polling timer live in `usePushRun`. */
export type RunKind = "loading" | "refused" | "confirm" | "running" | "pushed" | "failed" | "cancelled" | "creating" | "created" | "create_failed";

export interface RunState {
  kind: RunKind;
  /** What the checkout would push; null for a sheet opened on a push that was already running. */
  plan: PushPlan | null;
  /** The running push's id; null from the confirm until the core answers. */
  id: string | null;
  /** git's output as held so far: the last `lines.length` of the `seen` lines the core has had. */
  lines: string[];
  /** How many lines the core had when it last answered. It numbers them from the push's first and drops old ones, so this is what the next ask sends. */
  seen: number;
  /** The sentence of the end the state is in: a refusal, a failure, the push's own note when it pushed, GitHub's create failure. */
  reason: string | null;
  created: { number: number; url: string } | null;
}

/** The most lines held: what the core keeps (`PUSH_LINES`). */
export const PUSH_LINES = 2_000;

/** How often a running push is asked for its output. */
export const PUSH_POLL_MS = 250;

export const INITIAL_RUN: RunState = { kind: "loading", plan: null, id: null, lines: [], seen: 0, reason: null, created: null };

export type RunEvent =
  | { type: "plan"; plan: PushPlan }
  | { type: "plan_failed"; reason: string }
  | { type: "confirm"; create: boolean }
  | { type: "started"; id: string }
  | { type: "start_refused"; reason: string }
  | { type: "progress"; progress: PushProgress }
  | { type: "try_again" }
  | { type: "create_started" }
  | { type: "created"; number: number; url: string }
  | { type: "create_failed"; reason: string };

/**
 * The lines held after an answer. `prev` are the last lines of the `seen` the core had at the answer before; this answer's
 * lines begin at the core's line `progress.start`, which is `seen - 1` when the last line seen may have been redrawn. What was
 * held before `start` stays, the answer's lines follow, and no more than `PUSH_LINES` are kept.
 */
export function linesOf(prev: string[], seen: number, progress: PushProgress): string[] {
  const first = seen - prev.length;
  const keep = Math.min(prev.length, Math.max(0, progress.start - first));
  return [...prev.slice(0, keep), ...progress.lines].slice(-PUSH_LINES);
}

/** Whether the output keeps following its last line: only while the reader is at (or within a few pixels of) the end. */
export function shouldFollow(scrollTop: number, clientHeight: number, scrollHeight: number): boolean {
  return scrollHeight - scrollTop - clientHeight <= 8;
}

/** The whole flow, pure. An event that doesn't fit the state leaves it as it is. */
export function pushStep(s: RunState, e: RunEvent): RunState {
  switch (e.type) {
    case "plan":
      if (s.kind !== "loading") return s;
      return e.plan.refusal !== null ? { ...s, kind: "refused", plan: e.plan, reason: e.plan.refusal } : { ...s, kind: "confirm", plan: e.plan };
    case "plan_failed":
      return s.kind === "loading" ? { ...s, kind: "refused", reason: e.reason } : s;
    case "confirm":
      if (s.kind === "create_failed") return { ...s, kind: "creating", reason: null };
      if (s.kind !== "confirm") return s;
      if (e.create && s.plan?.to_push === 0) return { ...s, kind: "creating" };
      return { ...s, kind: "running", id: null, lines: [], seen: 0, reason: null };
    case "started":
      return s.kind === "running" && s.id === null ? { ...s, id: e.id } : s;
    case "start_refused":
      return s.kind === "running" && s.id === null ? { ...s, kind: "failed", reason: e.reason } : s;
    case "progress": {
      if (s.kind !== "running") return s;
      const lines = linesOf(s.lines, s.seen, e.progress);
      const seen = e.progress.total;
      switch (e.progress.state) {
        case "running":
          return { ...s, lines, seen };
        case "pushed":
          return { ...s, kind: "pushed", lines, seen, reason: e.progress.reason };
        case "failed":
          return { ...s, kind: "failed", lines, seen, reason: e.progress.reason ?? "The push failed." };
        case "cancelled":
          return { ...s, kind: "cancelled", lines, seen, reason: null };
      }
      return s;
    }
    case "try_again":
      return s.kind === "failed" ? { ...s, kind: "running", id: null, lines: [], seen: 0, reason: null } : s;
    case "create_started":
      return s.kind === "pushed" || s.kind === "confirm" || s.kind === "create_failed" ? { ...s, kind: "creating", reason: null } : s;
    case "created":
      return s.kind === "creating" ? { ...s, kind: "created", created: { number: e.number, url: e.url } } : s;
    case "create_failed":
      return s.kind === "creating" ? { ...s, kind: "create_failed", reason: e.reason } : s;
  }
}
