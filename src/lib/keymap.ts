export type Os = "windows" | "mac" | "linux";
/** The key with the platform's app modifier implied; `plain` overrides keys the plan leaves as written. */
export type Binding = { key: string; alt?: boolean; plain?: { win: string; mac: string } };
export type Command = { id: string; label: string; binding?: Binding; group: "Sessions" | "View" | "Tools" };

/** macOS: Cmd+key. Windows and Linux: Ctrl+Shift+key (docs/PLAN.md section 7, "Keyboard routing"). */
export function shortcutLabel(b: Binding, os: Os): string {
  if (b.plain) return os === "mac" ? b.plain.mac : b.plain.win;
  if (os === "mac") return `⌘${b.alt ? "⌥" : ""}${b.key}`;
  return `Ctrl+Shift+${b.alt ? "Alt+" : ""}${b.key}`;
}

export const COMMANDS: Command[] = [
  { id: "launcher", label: "Start a session", group: "Sessions", binding: { key: "T" } },
  { id: "focus-prompt", label: "Write a prompt to the active session", group: "Sessions", binding: { key: "M" } },
  { id: "next-waiting", label: "Go to the oldest waiting session", group: "Sessions", binding: { key: "N" } },
  { id: "close", label: "Close session", group: "Sessions", binding: { key: "W" } },
  { id: "reopen", label: "Reopen closed session", group: "Sessions", binding: { key: "T", alt: true } },
  { id: "rename", label: "Rename session", group: "Sessions", binding: { key: "F2", plain: { win: "F2", mac: "F2" } } },
  { id: "split", label: "Split: add a pane (up to 4)", group: "View", binding: { key: "\\" } },
  { id: "unsplit", label: "Remove this pane from the view", group: "View" },
  { id: "layout", label: "Change split layout", group: "View" },
  { id: "zoom-pane", label: "Zoom pane", group: "View", binding: { key: "Z" } },
  { id: "next-pane", label: "Next pane", group: "View", binding: { key: "]" } },
  { id: "prev-pane", label: "Previous pane", group: "View", binding: { key: "[" } },
  { id: "focus-mode", label: "Focus mode", group: "View" },
  { id: "sidebar", label: "Collapse sidebar to the rail", group: "View" },
  { id: "palette", label: "Command palette", group: "Tools", binding: { key: "K" } },
  { id: "history", label: "History", group: "Tools", binding: { key: "H" } },
  { id: "find", label: "Search in scrollback", group: "Tools", binding: { key: "F" } },
  { id: "global-search", label: "Search all sessions", group: "Tools", binding: { key: "F", alt: true } },
  { id: "bookmark", label: "Bookmark this point", group: "Tools", binding: { key: "B" } },
  { id: "cheatsheet", label: "Keyboard shortcuts", group: "Tools", binding: { key: "/" } },
  { id: "doctor", label: "Open Doctor", group: "Tools" },
  { id: "departures", label: "Open Departures board", group: "Tools" },
];

export function commandById(id: string): Command {
  const c = COMMANDS.find((x) => x.id === id);
  if (!c) throw new Error(`Unknown command: ${id}`);
  return c;
}
