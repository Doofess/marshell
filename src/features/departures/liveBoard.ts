import type { ApprovalRequest } from "../approval/types";
import type { RowModel } from "../sidebar/types";
import { fitRows, type BoardRow } from "./board";

export function refocusNeeded(s: { rowFocused: boolean; leaving: boolean; focusInside: boolean }): boolean {
  return s.rowFocused && !s.leaving && !s.focusInside;
}

export function openerUsable(opener: { isConnected: boolean } | null | undefined, body: unknown): boolean {
  return opener !== null && opener !== undefined && opener.isConnected && opener !== body;
}

/** Keep a window of whole rows ending at the card or active row when it would be hidden. */
export function fitLiveBoard(heightPx: number, groups: { gate: BoardRow[]; others: BoardRow[] }, card?: string, keep?: string) {
  const fit = fitRows(heightPx, groups, card !== undefined);
  const count = fit.gate.length + fit.others.length;
  const rows = [...groups.gate, ...groups.others];
  const index = rows.findIndex((r) => r.id === (card ?? keep));
  if (index < count || count === 0) return fit;
  const shown = rows.slice(index - count + 1, index + 1);
  const gate = new Set(groups.gate.map((r) => r.id));
  return { gate: shown.filter((r) => gate.has(r.id)), others: shown.filter((r) => !gate.has(r.id)), hidden: rows.length - count };
}

/** The board counts held requests even when a plain shell's sidebar status is idle. */
export function gateRows(rows: RowModel[], requests: ApprovalRequest[]): RowModel[] {
  const waits = new Map<string, number>();
  for (const request of requests) {
    if (request.state.phase === "pending") waits.set(request.tab, Math.max(waits.get(request.tab) ?? 0, request.waitingMs));
  }
  return rows.map((row) => {
    const waitingMs = waits.get(row.id);
    return waitingMs === undefined ? row : { ...row, status: row.status === "needs-question" ? "needs-question" : "needs-permission", waitingMs };
  });
}

/** Map insertion order records where each last copy lived before its session closed. */
export function heldRows(rows: RowModel[], kept: ReadonlyMap<string, RowModel>, stale: string[]): RowModel[] {
  const result = [...rows];
  const present = new Set(rows.map((r) => r.id));
  const missing = new Set(stale);
  let index = 0;
  for (const [id, row] of kept) {
    if (missing.has(id) && !present.has(id)) result.splice(index, 0, row);
    index++;
  }
  return result;
}

export function cardFor(requests: ApprovalRequest[], rowId: string): ApprovalRequest | undefined {
  return requests.filter((r) => r.tab === rowId && r.state.phase === "pending").sort((a, b) => b.waitingMs - a.waitingMs)[0];
}

export function activeIndex(shownIds: string[], activeId: string | null, lastIndex: number): number {
  const found = activeId === null ? -1 : shownIds.indexOf(activeId);
  return found >= 0 ? found : Math.max(0, Math.min(lastIndex, shownIds.length - 1));
}

export function focusAfter(action: "escape" | "jump" | "all-sessions" | "card-closed"): "opener" | "terminal" | "palette" | "board-row" {
  switch (action) {
    case "escape": return "opener";
    case "jump": return "terminal";
    case "all-sessions": return "palette";
    case "card-closed": return "board-row";
  }
}
