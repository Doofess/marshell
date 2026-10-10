import { commandById, shortcutLabel, type Os } from "../../lib/keymap";

/** Every empty state the plan lists: what is empty, one sentence, and how to start (good news has no action). */
export type EmptyStateCopy = { id: string; title: string; body: string; action?: string };

/** The copy for one system: the shortcut comes from the keymap, so a rebinding or another system changes it. */
export function emptyStatesFor(os: Os): EmptyStateCopy[] {
  const start = `Start one with ${shortcutLabel(commandById("launcher").binding!, os)}.`;
  return [
    { id: "no-sessions", title: "No sessions yet", body: start, action: "Start a session" },
    { id: "no-projects", title: "No git folders found", body: "Type a folder path, or choose a folder.", action: "Choose a folder" },
    { id: "no-history", title: "No history yet", body: "Finished sessions are kept here, with their transcript.", action: "Start a session" },
    { id: "no-agents", title: "No coding agents found", body: "Install Claude Code, Codex or Gemini, or continue with shells and tools.", action: "Continue with shells and tools" },
    { id: "no-matches", title: "Nothing matches", body: "Try fewer letters, or a session number.", action: "Clear the search" },
    { id: "doctor-clear", title: "All clear", body: "Nothing needs fixing." },
    { id: "no-notifications", title: "No banners yet", body: "Banners appear here after the first one." },
    { id: "departures-empty", title: "No sessions on the board", body: start, action: "Start a session" },
  ];
}

/** Windows 11 is the only tested system, so the review page shows its labels. */
export const EMPTY_STATES: EmptyStateCopy[] = emptyStatesFor("windows");
