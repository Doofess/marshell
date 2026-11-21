import type { PaneOpen } from "../../generated/PaneOpen";
import type { PaneOpenDone } from "../../generated/PaneOpenDone";
import type { PaneView } from "../../generated/PaneView";
import { addressFor, isPageUrl, pageTitle } from "../browser/address";
import { agentLabel } from "../sessions/sessionActions";
import { openedByText, openedText } from "./paneCommands";
import { lastSegment, openRefused, paneReducer, type PaneAction } from "./paneState";
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
  return { view: now, actions, done, said, news };
}
