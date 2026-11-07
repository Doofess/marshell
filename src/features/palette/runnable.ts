/** What the window holds right now, as far as the palette's commands need it. */
export type RunFacts = {
  /** A session is in focus. */
  focused: boolean;
  /** How many sessions the list shows. */
  sessions: number;
  /** A session or request is waiting for the person. */
  waiting: boolean;
  /** A closed session can be reopened. */
  closed: boolean;
  /** The expanded sidebar is shown, so the shell panel is there to open. */
  shellPanel: boolean;
  /** The sidebar can fold or open; a window too narrow for it can't open it. */
  sidebarToggle: boolean;
};

type Need = Exclude<keyof RunFacts, "sessions"> | "any session" | "two sessions";

const NEEDS: ReadonlyArray<[Need, readonly string[]]> = [
  ["focused", ["close", "rename", "focus-prompt", "find", "bookmark", "announce-status", "prompt-prev", "prompt-next", "move-up", "move-down"]],
  ["any session", ["go-to-session"]],
  ["two sessions", ["move-up", "move-down"]],
  ["waiting", ["next-waiting"]],
  ["closed", ["reopen"]],
  ["shellPanel", ["shell-panel"]],
  ["sidebarToggle", ["sidebar"]],
];

/** The palette commands that would do nothing now, each once; the palette leaves them out rather than list a row that does nothing. */
export function cannotRun(f: RunFacts): readonly string[] {
  const holds = (need: Need) => (need === "any session" ? f.sessions > 0 : need === "two sessions" ? f.sessions > 1 : f[need]);
  return [...new Set(NEEDS.filter(([need]) => !holds(need)).flatMap(([, ids]) => ids))];
}
