import type { PaneOpen } from "../../generated/PaneOpen";
import type { PaneOpenDone } from "../../generated/PaneOpenDone";
import type { PaneView } from "../../generated/PaneView";
import { addressFor, isPageUrl, pageTitle } from "../browser/address";
import { agentLabel } from "../sessions/sessionActions";
import { openedByText, openedText } from "./paneCommands";
import { lastSegment, openRefused, PANE_LIMITS, paneReducer, tabsOf, type PaneAction } from "./paneState";
import { panelOfPath, type PanelId } from "./panels";

/** What the window knows when a frame of requests arrives. */
export type OpensContext = {
  /** The sessions the window lists, with their project (the recent-file list is kept by project). */
  sessions: ReadonlyMap<string, { project: string }>;
  /** The session on screen. */
  shown: string | null;
  /** The panels in front of the person, for the session `shown`. */
  onScreen: readonly PanelId[];
  /** True for an address the shell would not open in a page (the same check a typed address meets). */
  refuses(url: string): boolean;
};

export type TakenOpens = {
  /** The pane after every request was applied. */
  view: PaneView;
  /** The actions that take the window's own state from the old view to `view`, in order. */
  actions: PaneAction[];
  /** The answers for the requests that were not in `seen`, in order. */
  done: PaneOpenDone[];
  /** One announcement per tab added or marked. */
  said: string[];
  /** The panels of the sessions that got a tab while the panel was not in front of the person, each once. */
  news: { session: string; panel: PanelId }[];
  /** The sessions where an open a hook asked for met the 32-tab limit, each once. */
  full: string[];
};

/**
 * Applies a frame of `marshell open` requests to the pane: each becomes a background tab in its session, or is refused, and each is
 * answered. A request whose id is in `seen` was applied before and is skipped, so a frame delivered twice applies once. Pure: the
 * caller dispatches `actions`, says `said`, and posts `done`.
 */
export function takeOpens(view: PaneView, opens: readonly PaneOpen[], seen: ReadonlySet<number>, ctx: OpensContext): TakenOpens {
  const actions: PaneAction[] = [];
  const done: PaneOpenDone[] = [];
  const said: string[] = [];
  const news: { session: string; panel: PanelId }[] = [];
  const full: string[] = [];
  let now = view;
  const answer = (id: number, refused: PaneOpenDone["refused"]) => done.push({ id, refused });

  for (const open of opens) {
    if (seen.has(open.id)) continue;
    const session = ctx.sessions.get(open.tab_id);
    if (!session) {
      answer(open.id, "session");
      continue;
    }
    let target = open.target;
    let title: string;
    if (open.kind === "browser") {
      // As the URL bar reads a typed address: `HTTP://EXAMPLE.COM/` is `http://example.com/`.
      const typed = addressFor(target);
      // `isPageUrl` holds the length a tab's address may have: written out, an address can be longer than it was typed.
      if (typed.kind !== "open" || !isPageUrl(typed.url) || ctx.refuses(typed.url)) {
        answer(open.id, "address");
        continue;
      }
      target = typed.url;
      title = pageTitle(null, target);
    } else if (open.kind === "file") {
      title = lastSegment(target);
    } else {
      answer(open.id, "address");
      continue;
    }
    if (openRefused(now, open.tab_id, target)) {
      answer(open.id, "limit");
      if (open.auto && !full.includes(open.tab_id)) full.push(open.tab_id);
      continue;
    }
    const panel = panelOfPath(target);
    const onScreen = ctx.shown === open.tab_id && ctx.onScreen.includes(panel);
    const action: PaneAction = { type: "openBackground", session: open.tab_id, kind: open.kind, target, by: open.by, project: session.project, onScreen };
    const next = paneReducer(now, action);
    answer(open.id, null);
    if (next === now) continue;
    actions.push(action);
    now = next;
    said.push(open.by === "shell" ? openedText(title, panel) : openedByText(agentLabel(open.by), title, panel));
    if (!onScreen && !news.some((n) => n.session === open.tab_id && n.panel === panel)) news.push({ session: open.tab_id, panel });
  }
  return { view: now, actions, done, said, news, full };
}

/**
 * Which of `full` to say the 32-tab limit for: a session is told once, and again only after it has had room. `told` is the sessions
 * told so far; the returned set is the one to keep. Pass an empty `full` when a frame refused nothing, so room is noticed.
 * `canSay` names the sessions the line can be announced for (the one on screen; every one when omitted): a full session it
 * refuses is neither said nor marked told, so it is heard when that session is on screen.
 */
export function limitNews(told: ReadonlySet<string>, full: readonly string[], pane: PaneView, canSay: (session: string) => boolean = () => true): { say: string[]; told: Set<string> } {
  const say = full.filter((s) => !told.has(s) && canSay(s));
  const next = new Set<string>();
  for (const s of told) if (full.includes(s) || tabsOf(pane, s).tabs.length >= PANE_LIMITS.tabs) next.add(s);
  for (const s of say) next.add(s);
  return { say, told: next };
}

/** What a drop on the side pane comes to: the actions to dispatch, the last file opened, and whether the 32-tab limit stopped it. */
export type DropPlan = { actions: PaneAction[]; last: string | null; refused: boolean };

/**
 * A drop of files on the pane: each file that passed its check (`ok`: it is a file the core can read; a folder is not) opens as a
 * tab in its own panel, in order. The reducer is followed here too, so a drop of many files meets the 32-tab limit at the right
 * file, and the files after it are left out. Pure: the caller dispatches `actions`, says the limit when `refused`, and announces
 * `last`.
 */
export function dropPlan(pane: PaneView, session: string, project: string, paths: readonly { path: string; ok: boolean }[]): DropPlan {
  const actions: PaneAction[] = [];
  let now = pane;
  let last: string | null = null;
  let refused = false;
  for (const { path, ok } of paths) {
    if (!ok) continue;
    if (openRefused(now, session, path)) {
      refused = true;
      break;
    }
    const action: PaneAction = { type: "open", session, project, target: { path } };
    now = paneReducer(now, action);
    actions.push(action);
    last = path;
  }
  return { actions, last, refused };
}
