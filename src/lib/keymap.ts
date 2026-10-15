import { DEFAULT_NAV_MODIFIER, navModifierLabel, type NavModifier } from "./navigation/modifier";

export type Os = "windows" | "mac" | "linux";
/** The shortcut family for the core's platform name ("windows", "macos" or "linux"). */
export const osOfPlatform = (platform: "windows" | "macos" | "linux"): Os => (platform === "macos" ? "mac" : platform);
/**
 * A key by its PHYSICAL position (`KeyboardEvent.code`: "KeyK", "Digit1", "F2", "Tab"), not by the character it types, so the
 * same shortcut sits on the same key of a French, German, Dvorak, Hebrew or Cyrillic keyboard. Punctuation never appears here.
 * By default the platform's app modifier is implied; `plain` is the key alone and `nav` is the navigation chord.
 * `shift` is Shift (plus Alt when `alt` is set) WITHOUT the app modifier: Alt+Shift+arrow moves a session in the list.
 */
export type Binding = { code: string; alt?: boolean; plain?: boolean; nav?: boolean; shift?: boolean };
export type Command = { id: string; label: string; binding?: Binding; group: "Sessions" | "View" | "Tools" };

const KEY_TEXT: Record<string, string> = { ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓" };

/** The text printed on the keycap for a physical key. */
export function keyText(code: string): string {
  return KEY_TEXT[code] ?? code.replace(/^Key/, "").replace(/^Digit/, "");
}

/** macOS: Cmd+key. Windows and Linux: Ctrl+Shift+key (docs/PLAN.md, "Keyboard navigation and shortcuts"). A navigation chord uses the navigation modifier. */
export function shortcutLabel(b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): string {
  const key = keyText(b.code);
  if (b.plain) return key;
  if (b.shift) return os === "mac" ? `${b.alt ? "⌥" : ""}⇧${key}` : `${b.alt ? "Alt+" : ""}Shift+${key}`;
  if (b.nav) return os === "mac" ? `${navModifierLabel(os, nav)}${key}` : `${navModifierLabel(os, nav)}+${key}`;
  if (os === "mac") return `⌘${b.alt ? "⌥" : ""}${key}`;
  return `Ctrl+Shift+${b.alt ? "Alt+" : ""}${key}`;
}

/**
 * Ctrl+Shift+1 to 9 (Cmd+1 to 9 on macOS) go to sessions 1 to 9; any other number goes through "Go to session" in the palette.
 * Ctrl+digit alone is not used: terminals read Ctrl+2 to Ctrl+7 as control codes (NUL, ESC, FS, GS, RS, US = readline's undo) and vim uses Ctrl+6.
 */
const SESSION_SHORTCUTS: Command[] = Array.from({ length: 9 }, (_, i) => ({
  id: `focus-session-${i + 1}`,
  label: `Go to session ${i + 1}`,
  group: "Sessions" as const,
  binding: { code: `Digit${i + 1}` },
}));

/** The navigation chords, in the order the cheat sheet shows them. */
export const NAV_COMMAND_IDS = ["nav-next-pane", "nav-none"] as const;

export const COMMANDS: Command[] = [
  { id: "launcher", label: "Start a session", group: "Sessions", binding: { code: "KeyT" } },
  { id: "focus-prompt", label: "Write a prompt to the active session", group: "Sessions", binding: { code: "KeyL" } },
  ...SESSION_SHORTCUTS,
  { id: "go-to-session", label: "Go to session… (type its number, e.g. 12)", group: "Sessions" },
  { id: "next-waiting", label: "Go to the oldest waiting session", group: "Sessions", binding: { code: "KeyN" } },
  { id: "close", label: "Close session", group: "Sessions", binding: { code: "KeyW" } },
  { id: "reopen", label: "Reopen closed session", group: "Sessions", binding: { code: "KeyT", alt: true } },
  { id: "rename", label: "Rename session", group: "Sessions", binding: { code: "F2", plain: true } },
  { id: "move-up", label: "Move session up the list", group: "Sessions", binding: { code: "ArrowUp", alt: true, shift: true } },
  { id: "move-down", label: "Move session down the list", group: "Sessions", binding: { code: "ArrowDown", alt: true, shift: true } },
  // Session modes, the same five for every agent CLI (docs/PLAN.md "Session modes"); unbound, so a person picks the keys.
  { id: "mode-manual", label: "Mode: Manual, ask before each change", group: "Sessions" },
  { id: "mode-plan", label: "Mode: Plan, read and plan without changing anything", group: "Sessions" },
  { id: "mode-auto-edit", label: "Mode: Auto-edit, edits in the project go ahead", group: "Sessions" },
  { id: "mode-full-auto", label: "Mode: Full auto, a safety check decides what asks", group: "Sessions" },
  { id: "mode-bypass", label: "Mode: Bypass, only risky commands ask", group: "Sessions" },
  { id: "split", label: "Split: add a pane (up to 4)", group: "View", binding: { code: "KeyD" } },
  { id: "unsplit", label: "Remove this pane from the view", group: "View" },
  { id: "layout", label: "Change split layout", group: "View" },
  { id: "zoom-pane", label: "Zoom pane", group: "View", binding: { code: "KeyP" } },
  { id: "nav-next-pane", label: "Select the next terminal on screen", group: "View", binding: { code: "Tab", nav: true } },
  { id: "nav-none", label: "Select no terminal", group: "View", binding: { code: "Backspace", nav: true } },
  { id: "focus-mode", label: "Focus mode", group: "View" },
  { id: "sidebar", label: "Collapse sidebar to the rail", group: "View" },
  { id: "palette", label: "Command palette", group: "Tools", binding: { code: "KeyK" } },
  { id: "shell-panel", label: "Show or focus the shell panel", group: "Tools", binding: { code: "KeyS" } },
  { id: "announce-status", label: "Read the focused session's status", group: "Tools", binding: { code: "KeyG" } },
  { id: "history", label: "History", group: "Tools", binding: { code: "KeyY" } },
  { id: "find", label: "Search in scrollback", group: "Tools", binding: { code: "KeyF" } },
  { id: "global-search", label: "Search all sessions", group: "Tools", binding: { code: "KeyF", alt: true } },
  { id: "bookmark", label: "Bookmark this point", group: "Tools", binding: { code: "KeyB" } },
  { id: "cheatsheet", label: "Keyboard shortcuts", group: "Tools", binding: { code: "KeyO" } },
  { id: "profiles", label: "Shell profiles…", group: "Tools" },
  { id: "doctor", label: "Open Doctor", group: "Tools" },
  { id: "departures", label: "Open Departures board", group: "Tools" },
];

export function commandById(id: string): Command {
  const c = COMMANDS.find((x) => x.id === id);
  if (!c) throw new Error(`Unknown command: ${id}`);
  return c;
}
