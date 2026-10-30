import type { Profile } from "../../generated/Profile";
import type { SessionInfo } from "../../generated/SessionInfo";
import type { SessionMenuFacts } from "../sidebar/sessionMenu";
import type { RowModel } from "../sidebar/types";
import { roomForPane } from "../window/viewState";
import { MODES } from "../modes/sessionModes";
import type { Mode } from "../sidebar/types";
import type { Endpoint } from "../../generated/Endpoint";
import { sessions } from "../../lib/api/sessions";
import { ENDED_FIRST, copyResumeCommand, openInSystemTerminal } from "./resumeActions";
import { offersModeChange, resumeBlockedReason } from "./resumeState";

/** What the window knows about a session, as the row menu's item list needs it. `shown` panes are on screen; `fit` is the most the window has room for. */
export function menuFacts(
  id: string,
  ctx: {
    infos: SessionInfo[];
    profiles: Profile[];
    rows: RowModel[];
    panes: (string | null)[];
    shown: number;
    fit: number;
    /** The modes the session's agent can switch to; absent means the five. */
    offered?: (agent: string) => readonly Mode[];
  },
): SessionMenuFacts {
  const onScreen = ctx.panes.slice(0, ctx.shown);
  const info = ctx.infos.find((i) => i.tab_id === id);
  const row = ctx.rows.find((r) => r.id === id);
  const offered = info?.agent && ctx.offered ? ctx.offered(info.agent) : MODES;
  // Mode ▸ only when there is a mode to move to: an agent mapping one mode, or none, has nothing to offer.
  const otherMode = offered.some((m) => m !== info?.mode);
  return {
    agent: info?.agent != null,
    modeChangeable: otherMode && (row === undefined || offersModeChange(row, info)),
    // Not where the Resume bar would be disabled (its profile was removed): an item that can't apply is left out, never offered to fail.
    resumable: info !== undefined && info.state.kind !== "running" && resumeBlockedReason(info.profile_id, ctx.profiles) === undefined,
    task: row?.task === true,
    muted: info?.muted === true,
    bookmarkable: info?.agent === "claude",
    inShownPane: onScreen.includes(id),
    pausable: info !== undefined && info.state.kind === "running" && !info.paused,
    paused: info !== undefined && info.state.kind === "running" && info.paused,
    paneAvailable: onScreen.includes(null) || roomForPane(ctx.panes.length, ctx.fit),
  };
}

/**
 * The row menu's "Copy resume command" and "Open in system terminal". Opening a running session is refused here, without asking the
 * core, so two CLIs never write the same conversation (decision 6): the toast says "End the session first."
 */
export async function runResumeItem(
  itemId: "copy-resume" | "open-outside",
  session: { tabId: string; running: boolean },
  deps: { endpoint: Endpoint; announce: (text: string) => void; write: (text: string) => Promise<void>; api?: Pick<typeof sessions, "resumeCommand" | "openOutside"> },
): Promise<void> {
  const api = deps.api ?? sessions;
  if (itemId === "copy-resume") return copyResumeCommand(() => api.resumeCommand(deps.endpoint, session.tabId), deps.write, deps.announce);
  if (session.running) return deps.announce(ENDED_FIRST);
  return openInSystemTerminal(() => api.openOutside(deps.endpoint, session.tabId), deps.announce);
}

/**
 * Where focus goes once a menu item has run (the focused item unmounted, so it would fall to the page). Rename and Review changes
 * are null: the rename box and the sheet take it themselves, and so does the close bar, which `confirms`.
 */
export function focusAfterMenu(itemId: string, confirms: boolean): "row" | "terminal" | null {
  switch (itemId) {
    case "resume":
    case "bookmark":
    case "copy-resume":
    case "open-outside":
    case "mute":
    case "unmute":
      return "row";
    case "split":
    case "continue":
      return "terminal";
    case "close":
      return confirms ? null : "terminal";
    default:
      return null;
  }
}

/** The mode a menu item or palette command names ("mode-plan"), or null for anything else (including "mode", which opens the menu). */
export const modeOfItem = (id: string): Mode | null => MODES.find((m) => id === `mode-${m}`) ?? null;
