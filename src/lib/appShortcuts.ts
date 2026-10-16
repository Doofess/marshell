import { findCommand, type KeyEventLike } from "./bindings";
import type { Os } from "./keymap";

/** A shortcut the app takes even while a terminal has focus. Everything else goes to the program in the terminal. */
export type SessionShortcut =
  | { kind: "launcher" }
  | { kind: "palette" }
  | { kind: "close" }
  | { kind: "reopen" }
  | { kind: "rename" }
  | { kind: "goto"; number: number }
  | { kind: "move"; by: -1 | 1 }
  | { kind: "prompt"; by: -1 | 1 }
  /** Add a pane (Ctrl+Shift+D). */
  | { kind: "split" }
  /** Zoom the focused pane, or restore the panes (Ctrl+Shift+P). */
  | { kind: "zoom" }
  /** The next terminal on screen, and selecting no terminal: the navigation modifier with Tab and Backspace. */
  | { kind: "nextPane" }
  | { kind: "nonePane" };

/** Escape while the busy-close bar is up cancels it, even with focus in the terminal (where the window takes the key first). */
export function cancelsCloseBar(e: { key: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean; isComposing?: boolean }, barOpen: boolean): boolean {
  return barOpen && e.key === "Escape" && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && !e.isComposing;
}

const FOCUS_SESSION = /^focus-session-([1-9])$/;

/** Which app shortcut a key press is, or null when it belongs to the terminal. Matching is by physical key (lib/bindings). */
export function sessionShortcut(e: KeyEventLike, os: Os): SessionShortcut | null {
  const id = findCommand(e, os)?.id;
  return id === undefined ? null : commandShortcut(id);
}

/** The shortcut a palette command id stands for, or null for a command the live window does not run yet. */
export function commandShortcut(id: string): SessionShortcut | null {
  switch (id) {
    case "launcher":
    case "palette":
    case "close":
    case "reopen":
    case "rename":
      return { kind: id };
    case "move-up":
      return { kind: "move", by: -1 };
    case "move-down":
      return { kind: "move", by: 1 };
    case "prompt-prev":
      return { kind: "prompt", by: -1 };
    case "prompt-next":
      return { kind: "prompt", by: 1 };
    case "split":
      return { kind: "split" };
    case "zoom-pane":
      return { kind: "zoom" };
    case "nav-next-pane":
      return { kind: "nextPane" };
    case "nav-none":
      return { kind: "nonePane" };
  }
  const n = FOCUS_SESSION.exec(id)?.[1];
  return n === undefined ? null : { kind: "goto", number: Number(n) };
}
