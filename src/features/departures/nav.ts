import { keyText } from "../../lib/keymap";
import type { BoardRow } from "./board";

/** Where focus lands in an opened request: on the card itself, or on Deny. Never on Allow, so a keystroke cannot allow by reflex. */
export type CardFocus = "title" | "deny";
export type BoardState = { active: number; card: string | null; focus?: CardFocus };

/**
 * Arrow keys move, Enter jumps to the session (never answers), Y opens the inline card on a row that needs you with focus on the
 * card, N opens it with focus on Deny (it never denies by itself), Esc closes it.
 */
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
      return row ? { state: { active: s.active, card: null }, jump: row.id } : { state: s };
    case "KeyY":
      return row?.gate ? { state: { ...s, card: row.id, focus: "title" } } : { state: s };
    case "KeyN":
      return row?.gate ? { state: { ...s, card: row.id, focus: "deny" } } : { state: s };
    case "Escape":
      return { state: { active: s.active, card: null } };
    default:
      return { state: s };
  }
}

/**
 * The letter to show for a shortcut that is a physical key: the one the user's own layout prints on it (the browser's keyboard
 * layout map, when it is available), else the key's own letter. On a German keyboard the key at KeyY prints Z.
 */
export function keyHint(code: string, layout?: ReadonlyMap<string, string>): string {
  return (layout?.get(code) ?? keyText(code)).toUpperCase();
}
