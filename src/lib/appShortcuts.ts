import { findCommand, type KeyEventLike } from "./bindings";
import type { Os } from "./keymap";

/** A shortcut the app takes even while a terminal has focus. Everything else goes to the program in the terminal. */
export type SessionShortcut =
  | { kind: "launcher" }
  | { kind: "close" }
  | { kind: "reopen" }
  | { kind: "rename" }
  | { kind: "goto"; number: number }
  | { kind: "move"; by: -1 | 1 };

const FOCUS_SESSION = /^focus-session-([1-9])$/;

/** Which app shortcut a key press is, or null when it belongs to the terminal. Matching is by physical key (lib/bindings). */
export function sessionShortcut(e: KeyEventLike, os: Os): SessionShortcut | null {
  const id = findCommand(e, os)?.id;
  if (id === undefined) return null;
  switch (id) {
    case "launcher":
    case "close":
    case "reopen":
    case "rename":
      return { kind: id };
    case "move-up":
      return { kind: "move", by: -1 };
    case "move-down":
      return { kind: "move", by: 1 };
  }
  const n = FOCUS_SESSION.exec(id)?.[1];
  return n === undefined ? null : { kind: "goto", number: Number(n) };
}
