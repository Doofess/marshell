import { ringTone, type RingTone } from "../../components/ContextRing/ContextRing";
import { formatCost, formatDuration, formatTokens } from "../sidebar/format";
import type { RowModel } from "../sidebar/types";

/**
 * The window's status strip (the status bar): the facts the user's own status line shows (model and effort, context, the two plan limits,
 * cost, session time, MCP health, skills, environment, ports), for the session of the focused pane. Built from known data only:
 * an unknown value leaves its item out, never a dash. It fits itself to the room by dropping the least important item first and
 * never wraps.
 */
export type PlanUsage = {
  fiveHour?: { pct: number; resetsInMs?: number };
  sevenDay?: { pct: number; resetsInMs?: number };
};

export type ItemId = "model" | "ctx" | "limit5" | "limit7" | "cost" | "time" | "dirty" | "mcp" | "version" | "skills" | "venv" | "ports";
/** meter: label, percentage, bar and detail. text: label and value. ok: healthy MCP. problem: a named MCP problem. */
export type ItemKind = "meter" | "text" | "ok" | "problem";

export type StripItem = {
  id: ItemId;
  kind: ItemKind;
  priority: number;
  /** A short word before the value ("ctx", "5h", "skills"). */
  label?: string;
  value: string;
  /** What shortens away first: used of size, when a limit resets, the effort. */
  detail?: string;
  pct?: number;
  tone?: RingTone;
  /** A tiny bar after the value. */
  bar?: boolean;
  /** The accessible name: what the item says in full words. */
  name: string;
  /** False for the three that always stay: model, context and the 5-hour limit. A problem never drops either. */
  droppable: boolean;
};
export type FitItem = StripItem & { compact: number };

/** Higher stays longer. A problem outranks everything, then the three core items, then the rest in the order the user's status line drops them. */
export const PRIORITY = { mcpProblem: 100, ctx: 90, limit5: 85, model: 80, limit7: 75, cost: 70, time: 65, mcpOk: 60, ports: 55, skills: 50, dirty: 45, version: 40, venv: 30 } as const;

const clampPct = (n: number) => Math.min(100, Math.max(0, n));
const known = (n: number | undefined): n is number => n !== undefined && Number.isFinite(n);
const percent = (p: number) => `${Math.round(p)}%`;

function limit(id: "limit5" | "limit7", label: string, words: string, w: { pct: number; resetsInMs?: number } | undefined): StripItem | null {
  if (!w || !known(w.pct)) return null;
  const pct = clampPct(w.pct);
  const resets = known(w.resetsInMs) && w.resetsInMs > 0 ? formatDuration(w.resetsInMs) : null;
  return {
    id,
    kind: "meter",
    priority: id === "limit5" ? PRIORITY.limit5 : PRIORITY.limit7,
    label,
    value: percent(pct),
    pct,
    tone: ringTone(pct),
    bar: true,
    ...(resets ? { detail: `resets in ${resets}` } : {}),
    name: `${words} limit ${percent(pct)} used${resets ? `, resets in ${resets}` : ""}`,
    droppable: id === "limit7",
  };
}

/** The items in the order they are read, left to right. A core item always comes first, so a separator never leads the line. */
export function statusItems(row: RowModel, plan: PlanUsage | undefined, ports: string[] = []): StripItem[] {
  const out: Array<StripItem | null> = [];
  if (row.model)
    out.push({
      id: "model",
      kind: "text",
      priority: PRIORITY.model,
      value: row.model,
      ...(row.effort ? { detail: `· ${row.effort}` } : {}),
      name: row.effort ? `Model ${row.model}, ${row.effort} effort` : `Model ${row.model}`,
      droppable: false,
    });

  if (known(row.contextPct)) {
    const pct = clampPct(row.contextPct);
    const u = row.usage;
    const tokens = u && known(u.inContext) && known(u.window) && u.window > 0 ? `${formatTokens(u.inContext)}/${formatTokens(u.window)}` : null;
    out.push({
      id: "ctx",
      kind: "meter",
      priority: PRIORITY.ctx,
      label: "ctx",
      value: percent(pct),
      pct,
      tone: ringTone(pct),
      bar: true,
      ...(tokens ? { detail: tokens } : {}),
      name: `Context ${percent(pct)} used${tokens ? `, ${formatTokens(u!.inContext)} of ${formatTokens(u!.window)} tokens` : ""}`,
      droppable: false,
    });
  }

  out.push(limit("limit5", "5h", "5-hour", plan?.fiveHour), limit("limit7", "7d", "7-day", plan?.sevenDay));

  if (known(row.usage?.costUsd))
    out.push({ id: "cost", kind: "text", priority: PRIORITY.cost, value: formatCost(row.usage!.costUsd), name: `Cost so far ${formatCost(row.usage!.costUsd)}`, droppable: true });

  if (known(row.sessionMs) && row.sessionMs > 0)
    out.push({ id: "time", kind: "text", priority: PRIORITY.time, value: formatDuration(row.sessionMs), name: `Session running ${formatDuration(row.sessionMs)}`, droppable: true });

  if (known(row.dirty) && row.dirty > 0)
    out.push({ id: "dirty", kind: "text", priority: PRIORITY.dirty, value: `~${row.dirty} changed`, name: `${row.dirty} uncommitted ${row.dirty === 1 ? "change" : "changes"}`, droppable: true });

  if (row.mcp) {
    const { connected, total, problem } = row.mcp;
    out.push(
      problem
        ? { id: "mcp", kind: "problem", priority: PRIORITY.mcpProblem, value: problem, name: `MCP problem: ${problem}, ${connected} of ${total} servers connected`, droppable: false }
        : {
            id: "mcp",
            kind: "ok",
            priority: PRIORITY.mcpOk,
            label: "mcp",
            value: connected === total ? String(connected) : `${connected} of ${total}`,
            name: connected === total ? `MCP servers: ${connected} connected` : `MCP servers: ${connected} of ${total} connected`,
            droppable: true,
          },
    );
  }

  if (row.version) out.push({ id: "version", kind: "text", priority: PRIORITY.version, value: `v${row.version}`, name: `Version ${row.version}`, droppable: true });
  if (known(row.skills) && row.skills > 0) out.push({ id: "skills", kind: "text", priority: PRIORITY.skills, label: "skills", value: String(row.skills), name: `${row.skills} skills`, droppable: true });
  if (row.venv) out.push({ id: "venv", kind: "text", priority: PRIORITY.venv, label: "venv", value: row.venv, name: `Python environment ${row.venv}`, droppable: true });

  const shown = ports.map((p) => p.split(":").pop()).filter((p): p is string => !!p && /^\d+$/.test(p));
  if (shown.length) out.push({ id: "ports", kind: "text", priority: PRIORITY.ports, value: shown.map((p) => `:${p}`).join(" "), name: `Listening on ${shown.map((p) => `port ${p}`).join(", ")}`, droppable: true });

  return out.filter((i): i is StripItem => i !== null);
}

const BAR = 4;
const SEPARATOR = 3;

/** An item's width in characters at a compaction level: 0 is everything, 1 drops the detail, 2 drops the bar too. */
export function itemWidth(i: StripItem, compact: number): number {
  let w = (i.label ? i.label.length + 1 : 0) + i.value.length;
  const hasBar = !!i.bar;
  const hasDetail = !!i.detail;
  if (hasBar && compact < (hasDetail ? 2 : 1)) w += BAR;
  if (hasDetail && compact < 1) w += 1 + i.detail!.length;
  return w;
}

const levels = (i: StripItem) => (i.detail ? 1 : 0) + (i.bar ? 1 : 0);

export function stripWidth(items: FitItem[]): number {
  if (!items.length) return 0;
  return items.reduce((sum, i) => sum + itemWidth(i, i.compact), 0) + SEPARATOR * (items.length - 1);
}

/**
 * Fits the items to `width` characters without wrapping. It degrades the least important item first: an item that has a
 * detail or a bar loses them (detail first), and one that has neither, or has nothing left to lose, is dropped. The three core
 * items and a named MCP problem are only ever shortened. The order is kept.
 */
export function fitItems(items: StripItem[], width: number): FitItem[] {
  const room = Number.isFinite(width) ? Math.max(0, width) : Infinity;
  let kept: FitItem[] = items.map((i) => ({ ...i, compact: 0 }));
  while (stripWidth(kept) > room) {
    const next = kept
      .filter((i) => i.compact < levels(i) || i.droppable)
      .sort((a, b) => a.priority - b.priority)[0];
    if (!next) break;
    if (next.compact < levels(next)) kept = kept.map((i) => (i === next ? { ...i, compact: i.compact + 1 } : i));
    else kept = kept.filter((i) => i !== next);
  }
  return kept;
}

/**
 * Where the strip's CSS (StatusStrip.css) hides each item, as the container's width in px: below the number the item is
 * gone. It follows the same order as fitItems degrades them, least important first, so server rendering fits without script.
 * Items missing here never hide: a problem, and the core model, context and 5-hour limit.
 */
export const HIDE_BELOW = { venv: 1320, version: 1220, dirty: 1130, skills: 1030, ports: 940, mcp: 840, time: 740, cost: 670, limit7: 500 } as const;
/** Where an item's detail, then its bar, goes. */
export const COMPACT_BELOW = { limit7Detail: 600, limit7Bar: 550, modelDetail: 460, limit5Detail: 410, limit5Bar: 360, ctxDetail: 320, ctxBar: 290 } as const;
