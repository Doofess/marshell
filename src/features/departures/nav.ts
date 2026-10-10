import type { BoardRow } from "./board";

export type BoardState = { active: number; card: string | null };

/** Arrow keys move, Enter jumps to the session (never answers), Y/N open the inline card on a row that needs you, Esc closes it. */
export function reduceBoard(s: BoardState, code: string, rows: BoardRow[]): { state: BoardState; jump?: string } {
  const last = Math.max(0, rows.length - 1);
  const row = rows[s.active];
  switch (code) {
    case "ArrowDown":
      return { state: { ...s, active: Math.min(last, s.active + 1) } };
    case "ArrowUp":
      return { state: { ...s, active: Math.max(0, s.active - 1) } };
    case "Home":
      return { state: { ...s, active: 0 } };
    case "End":
      return { state: { ...s, active: last } };
    case "Enter":
      return row ? { state: { ...s, card: null }, jump: row.id } : { state: s };
    case "KeyY":
    case "KeyN":
      return row?.gate ? { state: { ...s, card: row.id } } : { state: s };
    case "Escape":
      return { state: { ...s, card: null } };
    default:
      return { state: s };
  }
}
