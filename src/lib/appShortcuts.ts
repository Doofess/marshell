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
  | { kind: "nonePane" }
  /** Hide the sidebar, the rail and the prompt lines, or bring them back (Ctrl+Shift+I). */
  | { kind: "focusMode" }
  /** Fold the sidebar to the rail, or open it again. Has no key of its own: the palette runs it. */
  | { kind: "sidebar" }
  /** Show the shell panel, or focus its terminal (Ctrl+Shift+S). */
  | { kind: "shellPanel" }
  /** Open the Review sheet for the session in focus, when it is a task (Ctrl+Shift+R). The window owns the key even when there is nothing to review: the webview's hard reload must never run. */
  | { kind: "review" }
  /** Open the keyboard shortcuts sheet (Ctrl+Shift+O); pressed again while it is open, it closes it. */
  | { kind: "cheatsheet" }
  /** Show or hide the preview pane (Ctrl+Shift+Q; Cmd+Alt+Q on macOS). With no file opened yet it opens the palette to pick one. */
  | { kind: "preview" }
  /** "Open file…": the palette, to pick a file for the preview. Has no key of its own. */
  | { kind: "openFile" }
  /** Capture a screen region with the system's snipping tool and add the image to the focused session (Ctrl+Shift+X). Windows only for now. */
  | { kind: "capture" }
  /** Show or hide the History view in the main area (Ctrl+Shift+Y). */
  | { kind: "history" }
  /** Open the History view with the cursor in its search box (Ctrl+Shift+Alt+F). */
  | { kind: "globalSearch" }
  /** Bookmark the current point of the focused Claude Code session (Ctrl+Shift+B). */
  | { kind: "bookmark" }
  /** Focus the needs-you lane's target card, the oldest waiting request (Ctrl+Shift+N; Cmd+N on macOS). */
  | { kind: "nextWaiting" };

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
    case "focus-mode":
      return { kind: "focusMode" };
    case "sidebar":
      return { kind: "sidebar" };
    case "shell-panel":
      return { kind: "shellPanel" };
    case "review-task":
      return { kind: "review" };
    case "cheatsheet":
      return { kind: "cheatsheet" };
    case "preview":
      return { kind: "preview" };
    case "open-file":
      return { kind: "openFile" };
    case "capture":
      return { kind: "capture" };
    case "next-waiting":
      return { kind: "nextWaiting" };
    case "bookmark":
      return { kind: "bookmark" };
    case "history":
      return { kind: "history" };
    case "global-search":
      return { kind: "globalSearch" };
  }
  const n = FOCUS_SESSION.exec(id)?.[1];
  return n === undefined ? null : { kind: "goto", number: Number(n) };
}
