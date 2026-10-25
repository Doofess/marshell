import { isTool, kindName, type SessionKind } from "../../components/AgentMark/kinds";
import type { GlyphKind } from "../../components/StatusGlyph/glyphs";
import type { RowModel } from "../sidebar/types";

/** The board's columns, in order. Status and waiting are the two that flip. */
export const COLUMNS = ["STATUS", "SESSION", "PROJECT / BRANCH", "AGENT", "WAITING", "CTX", "RECAP"] as const;

export type BoardRow = {
  id: string;
  number: number;
  name: string;
  where: string;
  agent: SessionKind;
  agentName: string;
  status: GlyphKind;
  statusText: string;
  waitingText: string;
  ctxText: string;
  recap: string;
  /** Needs the user: shown in the "Gate calls" group at the top. */
  gate: boolean;
  waitingMs?: number;
};

export const ROW_PX = 44;
export const HEAD_PX = 36;
export const GROUP_PX = 28;

/** Status in capitals; a plain terminal says what a terminal does (at the prompt, running, exited). */
export function statusText(status: GlyphKind, manual: boolean): string {
  switch (status) {
    case "needs-permission":
    case "needs-question":
      return "NEEDS YOU";
    case "working":
      return "WORKING";
    case "running":
      return "RUNNING";
    case "idle":
      return manual ? "AT PROMPT" : "IDLE";
    case "done-unseen":
    case "done-seen":
      return "DONE";
    case "error":
      return "ERROR";
    case "stuck":
      return "STUCK";
    case "ended":
      return manual ? "EXITED" : "ENDED";
    case "limited":
      return "LIMITED";
    case "paused":
      return "PAUSED";
  }
}

/** Whole minutes, capitals, and an en dash when the time is not known. */
export function waitingText(ms?: number): string {
  if (ms === undefined) return "–";
  if (ms < 60_000) return "<1 MIN";
  const min = Math.floor(ms / 60_000);
  if (min < 60) return `${min} MIN`;
  return `${Math.floor(min / 60)} H ${String(min % 60).padStart(2, "0")} M`;
}

export function boardRows(rows: RowModel[]): BoardRow[] {
  return rows.map((r) => {
    const manual = isTool(r.agent);
    return {
      id: r.id,
      number: r.number,
      name: r.name,
      where: r.branch ? `${r.project} · ${r.branch}` : r.project,
      agent: r.agent,
      agentName: kindName(r.agent, r.agentName),
      status: r.status,
      statusText: statusText(r.status, manual),
      waitingText: waitingText(r.waitingMs),
      ctxText: manual || r.contextPct === undefined ? "–" : `${Math.round(r.contextPct)}%`,
      recap: r.recap?.text ?? "–",
      gate: r.status === "needs-permission" || r.status === "needs-question",
      waitingMs: r.waitingMs,
    };
  });
}

/** Gate calls (oldest waiting first), then everything else by session number. */
export function splitGroups(rows: BoardRow[]): { gate: BoardRow[]; others: BoardRow[] } {
  const gate = rows.filter((r) => r.gate).sort((a, b) => (b.waitingMs ?? 0) - (a.waitingMs ?? 0) || a.number - b.number);
  const others = rows.filter((r) => !r.gate).sort((a, b) => a.number - b.number);
  return { gate, others };
}

/** Keep the held order and groups; new rows join the end of their own group. */
export function holdGroups(groups: { gate: BoardRow[]; others: BoardRow[] }, hold: { order: string[]; gate: string[] }): { gate: BoardRow[]; others: BoardRow[] } {
  const rows = new Map([...groups.gate, ...groups.others].map((r) => [r.id, r]));
  const heldGate = new Set(hold.gate);
  const gate: BoardRow[] = [];
  const others: BoardRow[] = [];
  for (const id of hold.order) {
    const row = rows.get(id);
    if (!row) continue;
    (heldGate.has(id) ? gate : others).push(row);
    rows.delete(id);
  }
  for (const row of rows.values()) (row.gate ? gate : others).push(row);
  return { gate, others };
}

/** The "+N more sessions" line. */
export const MORE_PX = 44;
/** An open request (the approve card) under its row, which is taller than any row. */
export const CARD_PX = 220;

/** Whole rows only: the board never shows a partial row, counts the ones that did not fit, and keeps room for the "+N more" line and an open request. */
export function fitRows(heightPx: number, g: { gate: BoardRow[]; others: BoardRow[] }, cardOpen = false): { gate: BoardRow[]; others: BoardRow[]; hidden: number } {
  const total = g.gate.length + g.others.length;
  const groups = (g.gate.length ? 1 : 0) + (g.others.length ? 1 : 0);
  const base = HEAD_PX + groups * GROUP_PX + (cardOpen ? CARD_PX : 0);
  const rowsFor = (px: number) => Math.max(0, Math.floor(px / ROW_PX));
  // Everything fits: no "+N more" line. Otherwise one line of the room goes to it.
  const room = total <= rowsFor(heightPx - base) ? total : rowsFor(heightPx - base - MORE_PX);
  const gate = g.gate.slice(0, room);
  const others = g.others.slice(0, Math.max(0, room - gate.length));
  return { gate, others, hidden: total - gate.length - others.length };
}

/** The height the board takes for a fit: the header, the group lines that show, the rows, the "+N more" line and an open request. */
export function boardHeight(f: { gate: BoardRow[]; others: BoardRow[]; hidden: number }, cardOpen: boolean): number {
  return HEAD_PX + (f.gate.length ? GROUP_PX : 0) + (f.others.length ? GROUP_PX : 0) + (f.gate.length + f.others.length) * ROW_PX + (f.hidden > 0 ? MORE_PX : 0) + (cardOpen ? CARD_PX : 0);
}

/** The accessible name: number first, then every value the row shows, so label-in-name holds. */
export const boardRowLabel = (r: BoardRow): string =>
  `Session ${r.number}, ${r.name}, ${r.statusText.toLowerCase()}, ${r.where}, ${r.agentName}, waiting ${r.waitingText.toLowerCase()}, context ${r.ctxText}, ${r.recap}`;
