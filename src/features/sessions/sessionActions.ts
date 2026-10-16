import { AGENT_NAMES, isAgentId } from "../../components/AgentMark/agents";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { ApiError } from "../../lib/api/client";
import type { sessions } from "../../lib/api/sessions";
import type { SessionShortcut } from "../../lib/appShortcuts";
import { moveBy } from "../sidebar/reorder";
import { lastSegment } from "./toRow";

/** How long the "Closed <name>." toast stays, in milliseconds. */
export const TOAST_MS = 10_000;

/** A running agent session may be mid-task, so closing it asks first. A shell, or any session that has exited, closes at once. */
export const needsCloseConfirm = (info: SessionInfo): boolean => info.agent !== null && info.state.kind === "running";

/** The names the close bar uses; the sidebar's own mark names ("OpenAI Codex") are longer than the CLIs are known by. */
const CLOSE_BAR_NAMES: Record<string, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity" };

/** "Claude Code may be mid-task in my-app." The project is the last folder of the session's cwd. */
export function closeBarText(info: SessionInfo): string {
  const agent = info.agent ?? "";
  const name = CLOSE_BAR_NAMES[agent] ?? (isAgentId(agent) ? AGENT_NAMES[agent] : agent);
  return `${name} may be mid-task in ${lastSegment(info.cwd)}.`;
}

/** The tab of the session whose NUMBER is `number` (not the one in that position), or undefined. */
export const tabIdOfNumber = (infos: SessionInfo[], number: number): string | undefined => infos.find((i) => i.number === number)?.tab_id;

export type Toast = { id: number; text: string; /** Set when the toast offers Reopen for this closed tab. */ reopenId?: string };
export type ActionsState = {
  toast: Toast | null;
  /** The tab whose close bar is up. */
  closing: string | null;
  /** The tab whose name is being edited. */
  renaming: string | null;
  /** Bumped when focus should go back to the terminal. */
  focusToken: number;
};

type Api = Pick<typeof sessions, "close" | "reopen" | "rename" | "reorder">;
type Context = {
  endpoint: Endpoint | null;
  infos: SessionInfo[];
  selectedId: string | undefined;
  /** False in the rail and focus layouts, which have no row to edit. Left out means expanded. */
  sidebarExpanded?: boolean;
};
type Deps = { api: Api; context: () => Context; select: (tabId: string) => void };

const reason = (e: unknown): string => (e instanceof ApiError ? e.message : String(e));

/**
 * What the person does to the session list from the keyboard or the sidebar: close (with the busy-agent bar), reopen, rename,
 * reorder, go to a session. It sends the request and shows a toast on a refusal; the session list itself only changes with the
 * core's next snapshot, so nothing here is optimistic. Plain class so it can be tested without a DOM; LiveApp wraps it.
 */
export class SessionActions {
  private state: ActionsState = { toast: null, closing: null, renaming: null, focusToken: 0 };
  private listeners = new Set<() => void>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private toastId = 0;
  /** Tabs with a close request in flight. */
  private closingNow = new Set<string>();

  constructor(private deps: Deps) {}

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getState = (): ActionsState => this.state;

  private set(patch: Partial<ActionsState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l());
  }

  private refocus(): number {
    return this.state.focusToken + 1;
  }

  /** Stops the toast timer. Call when the window goes away. */
  dispose(): void {
    clearTimeout(this.timer);
  }

  private showToast(text: string, reopenId?: string): void {
    clearTimeout(this.timer);
    this.set({ toast: { id: ++this.toastId, text, reopenId } });
    this.timer = setTimeout(() => this.dismissToast(), TOAST_MS);
  }

  dismissToast = (): void => {
    clearTimeout(this.timer);
    if (this.state.toast) this.set({ toast: null });
  };

  /** Runs the shortcut the app took from the terminal. The launcher is the window's own business. */
  handle(shortcut: SessionShortcut): void {
    switch (shortcut.kind) {
      case "close":
        void this.requestClose();
        break;
      case "reopen":
        void this.reopenLast();
        break;
      case "rename":
        this.startRename();
        break;
      case "goto": {
        const id = tabIdOfNumber(this.deps.context().infos, shortcut.number);
        if (id !== undefined) this.deps.select(id);
        break;
      }
      case "move": {
        const { infos, selectedId } = this.deps.context();
        const next = selectedId === undefined ? null : moveBy(infos.map((i) => i.tab_id), selectedId, shortcut.by);
        if (next) this.reorder(next);
        break;
      }
      // The window opens these, and the terminal jumps between its own prompts. The panes belong to the window's view state.
      case "launcher":
      case "palette":
      case "prompt":
      case "split":
      case "zoom":
      case "nextPane":
      case "nonePane":
      case "focusMode":
      case "sidebar":
        break;
    }
  }

  /** Ctrl+Shift+W on the selected session: closes it, or puts the bar up when an agent may be mid-task. */
  async requestClose(): Promise<void> {
    const { infos, selectedId } = this.deps.context();
    const info = infos.find((i) => i.tab_id === selectedId);
    // While the bar is up, the key does nothing: closing must stay a deliberate press on "Close now".
    if (!info || this.state.closing === info.tab_id || this.closingNow.has(info.tab_id)) return;
    if (needsCloseConfirm(info)) this.set({ closing: info.tab_id });
    else await this.close(info);
  }

  /**
   * "Close now" on the bar. The bar (and the button that had focus) goes away, so focus is handed back to the terminal whether
   * or not the close works; when it was the last session the launcher opens and takes the keys itself.
   */
  async confirmClose(): Promise<void> {
    const { infos } = this.deps.context();
    const info = infos.find((i) => i.tab_id === this.state.closing);
    this.set({ closing: null, focusToken: this.refocus() });
    if (info) await this.close(info);
  }

  /** Cancel or Escape on the bar: the session stays, and focus goes back to its terminal. */
  cancelClose(): void {
    if (this.state.closing !== null) this.set({ closing: null, focusToken: this.refocus() });
  }

  private async close(info: SessionInfo): Promise<void> {
    const { endpoint } = this.deps.context();
    // One close per tab at a time: a second press must not turn the first one's "Closed" receipt into "Couldn't close".
    if (!endpoint || this.closingNow.has(info.tab_id)) return;
    this.closingNow.add(info.tab_id);
    try {
      await this.deps.api.close(endpoint, info.tab_id);
      this.showToast(`Closed ${info.name}.`, info.tab_id);
    } catch {
      this.showToast(`Couldn't close ${info.name}.`);
    } finally {
      this.closingNow.delete(info.tab_id);
    }
  }

  /** Ctrl+Shift+Alt+T: the most recently closed session. */
  async reopenLast(): Promise<void> {
    await this.reopen(undefined);
  }

  /** The toast's Reopen: that very tab. */
  async reopenFromToast(): Promise<void> {
    const id = this.state.toast?.reopenId;
    if (id !== undefined) await this.reopen(id);
  }

  private async reopen(tabId: string | undefined): Promise<void> {
    const { endpoint } = this.deps.context();
    if (!endpoint) return;
    try {
      // No id means "the most recent": the core takes the absence of an id, not an empty one.
      await (tabId === undefined ? this.deps.api.reopen(endpoint) : this.deps.api.reopen(endpoint, tabId));
      this.dismissToast();
    } catch (e) {
      this.showToast(e instanceof ApiError && e.status === 404 ? "Nothing to reopen." : `Couldn't reopen the session: ${reason(e)}`);
    }
  }

  /** F2 on the selected session. It does nothing where the sidebar is not expanded: there is no row to turn into a text box. */
  startRename(): void {
    const { infos, selectedId, sidebarExpanded = true } = this.deps.context();
    if (sidebarExpanded && infos.some((i) => i.tab_id === selectedId)) this.set({ renaming: selectedId ?? null });
  }

  async commitRename(tabId: string, name: string): Promise<void> {
    const { endpoint } = this.deps.context();
    this.set({ renaming: null, focusToken: this.refocus() });
    if (!endpoint) return;
    try {
      await this.deps.api.rename(endpoint, tabId, name);
    } catch (e) {
      this.showToast(`Couldn't rename the session: ${reason(e)}`);
    }
  }

  /** Escape (refocus the terminal) or blur (the person went elsewhere; leave focus where they put it). */
  cancelRename({ refocus = true }: { refocus?: boolean } = {}): void {
    if (this.state.renaming === null) return;
    this.set({ renaming: null, ...(refocus ? { focusToken: this.refocus() } : {}) });
  }

  /** Sends a new order (a drop, or a keyboard move). The list on screen follows the core's next snapshot. */
  reorder(tabIds: string[]): void {
    const { endpoint } = this.deps.context();
    if (!endpoint) return;
    this.deps.api.reorder(endpoint, tabIds).catch((e) => this.showToast(`Couldn't move the session: ${reason(e)}`));
  }
}
