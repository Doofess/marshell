import { DEFAULT_NAV_MODIFIER, navModifierLabel, type NavModifier } from "./navigation/modifier";

export type Os = "windows" | "mac" | "linux";
/** The shortcut family for the core's platform name ("windows", "macos" or "linux"). */
export const osOfPlatform = (platform: "windows" | "macos" | "linux"): Os => (platform === "macos" ? "mac" : platform);
/**
 * A key by its PHYSICAL position (`KeyboardEvent.code`: "KeyK", "Digit1", "F2", "Tab"), not by the character it types, so the
 * same shortcut sits on the same key of a French, German, Dvorak, Hebrew or Cyrillic keyboard. Punctuation never appears here.
 * By default the platform's app modifier is implied; `plain` is the key alone and `nav` is the navigation chord.
 * `shift` is Shift (plus Alt when `alt` is set) WITHOUT the app modifier: Alt+Shift+arrow moves a session in the list.
 * `mac` replaces the whole binding on macOS, for a key the system already owns there: Cmd+Q is Quit, so the preview is Cmd+Alt+Q.
 */
export type Binding = { code: string; alt?: boolean; plain?: boolean; nav?: boolean; shift?: boolean; mac?: Binding };
export type Command = { id: string; label: string; binding?: Binding; group: "Session" | "Terminal" | "Window" | "App" };

const KEY_TEXT: Record<string, string> = { ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓" };

/** The text printed on the keycap for a physical key. */
export function keyText(code: string): string {
  return KEY_TEXT[code] ?? code.replace(/^Key/, "").replace(/^Digit/, "");
}

/** macOS: Cmd+key. Windows and Linux: Ctrl+Shift+key (docs/PLAN.md, "Keyboard navigation and shortcuts"). A navigation chord uses the navigation modifier. */
export function shortcutLabel(b: Binding, os: Os, nav: NavModifier = DEFAULT_NAV_MODIFIER): string {
  if (os === "mac" && b.mac) return shortcutLabel(b.mac, os, nav);
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
  group: "Session" as const,
  binding: { code: `Digit${i + 1}` },
}));

/** The navigation chords, in the order the cheat sheet shows them. */
export const NAV_COMMAND_IDS = ["nav-next-pane", "nav-none"] as const;

export const COMMANDS: Command[] = [
  { id: "launcher", label: "Start a session", group: "Session", binding: { code: "KeyT" } },
  { id: "focus-prompt", label: "Write a prompt to the active session", group: "Session", binding: { code: "KeyL" } },
  ...SESSION_SHORTCUTS,
  // Opens the launcher with the Administrator chip on. Windows only; the window hides it elsewhere.
  { id: "new-session-admin", label: "New session as administrator", group: "Session" },
  { id: "go-to-session", label: "Go to session… (type its number, e.g. 12)", group: "Session" },
  { id: "next-waiting", label: "Go to the oldest waiting session", group: "Session", binding: { code: "KeyN" } },
  { id: "close", label: "Close session", group: "Session", binding: { code: "KeyW" } },
  { id: "reopen", label: "Reopen closed session", group: "Session", binding: { code: "KeyT", alt: true } },
  { id: "rename", label: "Rename session", group: "Session", binding: { code: "F2", plain: true } },
  { id: "resume", label: "Resume this session", group: "Session" },
  { id: "review-task", label: "Review this task's changes", group: "Session", binding: { code: "KeyR" } },
  // A task whose folder was deleted by hand has no session to review it from; this opens its removal.
  { id: "clean-up-task", label: "Clean up a missing task…", group: "Session" },
  { id: "move-up", label: "Move session up the list", group: "Session", binding: { code: "ArrowUp", alt: true, shift: true } },
  { id: "move-down", label: "Move session down the list", group: "Session", binding: { code: "ArrowDown", alt: true, shift: true } },
  // Session modes, the same five for every agent CLI (docs/PLAN.md "Session modes"); unbound, so a person picks the keys.
  { id: "pause", label: "Pause session", group: "Session" },
  { id: "continue", label: "Continue session", group: "Session" },
  { id: "mode-manual", label: "Mode: Manual, ask before each change", group: "Session" },
  { id: "mode-plan", label: "Mode: Plan, read and plan without changing anything", group: "Session" },
  { id: "mode-auto-edit", label: "Mode: Auto-edit, edits in the project go ahead", group: "Session" },
  { id: "mode-full-auto", label: "Mode: Full auto, a safety check decides what asks", group: "Session" },
  { id: "mode-bypass", label: "Mode: Bypass, only risky commands ask", group: "Session" },
  { id: "announce-status", label: "Read the focused session's status", group: "Session", binding: { code: "KeyG" } },
  { id: "copy-recording-path", label: "Copy this session's recording path", group: "Session" },
  { id: "run-script", label: "Run the repo's run script", group: "Session" },
  { id: "stop-script", label: "Stop the repo's run script", group: "Session" },
  { id: "edit-references", label: "Edit this project's references", group: "Session" },
  { id: "nav-next-pane", label: "Select the next terminal on screen", group: "Terminal", binding: { code: "Tab", nav: true } },
  { id: "nav-none", label: "Select no terminal", group: "Terminal", binding: { code: "Backspace", nav: true } },
  { id: "find", label: "Search in scrollback", group: "Terminal", binding: { code: "KeyF" } },
  { id: "bookmark", label: "Bookmark this point", group: "Terminal", binding: { code: "KeyB" } },
  // Windows only for now (the snipping tool); elsewhere it runs and says so. Cmd+X is Cut on macOS, so it takes Cmd+Alt+X there.
  { id: "capture", label: "Capture screen region", group: "Terminal", binding: { code: "KeyX", mac: { code: "KeyX", alt: true } } },
  { id: "prompt-prev", label: "Jump to the previous prompt", group: "Terminal", binding: { code: "PageUp" } },
  { id: "prompt-next", label: "Jump to the next prompt", group: "Terminal", binding: { code: "PageDown" } },
  { id: "split", label: "Split: add a pane (up to 4)", group: "Window", binding: { code: "KeyD" } },
  { id: "unsplit", label: "Remove this pane from the view", group: "Window" },
  { id: "layout", label: "Change split layout", group: "Window" },
  { id: "zoom-pane", label: "Zoom pane", group: "Window", binding: { code: "KeyP" } },
  { id: "focus-mode", label: "Focus mode", group: "Window", binding: { code: "KeyI" } },
  // Cmd+Q is Quit on macOS, so the preview takes Cmd+Alt+Q there.
  { id: "preview", label: "Preview", group: "Window", binding: { code: "KeyQ", mac: { code: "KeyQ", alt: true } } },
  { id: "open-file", label: "Open file…", group: "Window" },
  { id: "sidebar", label: "Collapse sidebar to the rail", group: "Window" },
  { id: "group-by-project", label: "Group sessions by project", group: "Window" },
  { id: "shell-panel", label: "Show or focus the shell panel", group: "Window", binding: { code: "KeyS" } },
  { id: "palette", label: "Command palette", group: "App", binding: { code: "KeyK" } },
  { id: "history", label: "History", group: "App", binding: { code: "KeyY" } },
  { id: "cheatsheet", label: "Keyboard shortcuts", group: "App", binding: { code: "KeyO" } },
  { id: "settings", label: "Settings…", group: "App" },
  { id: "profiles", label: "Settings: Shell profiles…", group: "App" },
  { id: "appearance", label: "Settings: Appearance…", group: "App" },
  { id: "agents", label: "Settings: Agents…", group: "App" },
  { id: "diagnostics", label: "Settings: Diagnostics…", group: "App" },
  { id: "notifications", label: "Settings: Notifications…", group: "App" },
  { id: "history-settings", label: "Settings: History…", group: "App" },
  { id: "pause-notifications", label: "Pause or resume notifications", group: "App" },
  { id: "doctor", label: "Open Doctor", group: "App" },
  { id: "departures", label: "Open Departures board", group: "App" },
];

/** Commands the palette leaves out until a person types: the menu button holds them. */
export const MOVED_COMMANDS: readonly string[] = ["settings", "appearance", "agents", "profiles", "notifications", "history-settings", "diagnostics", "history", "doctor", "departures"];

export function commandById(id: string): Command {
  const c = COMMANDS.find((x) => x.id === id);
  if (!c) throw new Error(`Unknown command: ${id}`);
  return c;
}
