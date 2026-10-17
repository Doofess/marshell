import { AGENT_NAMES, isAgentId } from "../../components/AgentMark/agents";
import type { Endpoint } from "../../generated/Endpoint";
import type { SessionInfo } from "../../generated/SessionInfo";
import { ApiError } from "../../lib/api/client";
import type { sessions } from "../../lib/api/sessions";
import type { SessionShortcut } from "../../lib/appShortcuts";
import { moveWithinGroup } from "../sidebar/grouping";
import { moveBy } from "../sidebar/reorder";
import type { RowModel } from "../sidebar/types";
import { modeLabel } from "../window/SessionHeader";
import type { Mode } from "../sidebar/types";
import { resumeCommand } from "./resumeCommand";
import { lastSegment } from "./toRow";

/** How long the "Closed <name>." toast stays, in milliseconds. */
export const TOAST_MS = 10_000;

/** A running agent session may be mid-task, so closing it asks first. A shell, or any session that has exited, closes at once. */
export const needsCloseConfirm = (info: SessionInfo): boolean => info.agent !== null && info.state.kind === "running";

/** The names the close bar uses; the sidebar's own mark names ("OpenAI Codex") are longer than the CLIs are known by. */
const CLOSE_BAR_NAMES: Record<string, string> = { claude: "Claude Code", codex: "Codex", antigravity: "Antigravity" };

/** An agent's name in a sentence ("Claude Code"): the only thing about the agent the sheets use. An unknown agent is named by its id. */
export function agentLabel(agent: string): string {
  return CLOSE_BAR_NAMES[agent] ?? (isAgentId(agent) ? AGENT_NAMES[agent] : agent);
}

/** "Claude Code may be mid-task in my-app." The project is the last folder of the session's cwd. */
export function closeBarText(info: SessionInfo): string {
  return `${agentLabel(info.agent ?? "")} may be mid-task in ${lastSegment(info.cwd)}.`;
}

/** "Restart 3 billing in Bypass? It ends and runs claude --continue --permission-mode bypassPermissions." The command is what the core runs. */
export function restartBarText(info: SessionInfo, mode: Mode, working: boolean): string {
  const text = `Restart ${info.number} ${info.name} in ${modeLabel(mode)}? It ends and runs ${resumeCommand(info.agent, info.profile_id, mode)}.`;
  return working ? `${text} ${agentLabel(info.agent ?? "")} may be mid-task.` : text;
}

/**
 * Whether the restart bar has nothing left to ask: its session is gone, a close question is up, or the focus is on
 * another session. `showing` is a session being brought on screen for the bar; until it is focused, that is not a move away.
 */
export function restartBarStale(s: { id: string; closing: string | null; focusedId: string | undefined; ids: string[]; showing: string | null }): boolean {
  if (!s.ids.includes(s.id) || s.closing !== null) return true;
  return s.id !== s.focusedId && s.id !== s.showing;
}

/** "Pause 3 billing? It freezes the program and everything it started. Network connections can time out while it's paused." */
export function pauseBarText(info: SessionInfo): string {
  return `Pause ${info.number} ${info.name}? It freezes the program and everything it started. Network connections can time out while it's paused.`;
}

/** The typing bar's words: shown when keys are typed into a paused session, which sends none of them. */
export const PAUSED_TYPING_TEXT = "Paused. Continue to type.";

/**
 * Whether a Pause bar has nothing left to ask. The restart bar's rules (session gone, a close question up, focus elsewhere), and
 * the snapshot already agreeing: the confirm bar is moot once the session is paused, the typing bar once it is running again.
 */
export function pauseBarStale(s: { id: string; closing: string | null; focusedId: string | undefined; ids: string[]; showing: string | null; kind: "confirm" | "typing"; paused: boolean }): boolean {
  return restartBarStale(s) || (s.kind === "confirm" ? s.paused : !s.paused);
}

/** The tab of the session whose NUMBER is `number` (not the one in that position), or undefined. */
export const tabIdOfNumber = (infos: SessionInfo[], number: number): string | undefined => infos.find((i) => i.number === number)?.tab_id;

export type Toast = { id: number; text: string; /** Set when the toast offers Reopen for this closed tab. */ reopenId?: string };
export type ActionsState = {
  toast: Toast | null;
  /** The tab whose close bar is up. */
  closing: string | null;
  /** The tab whose restart bar is up, and the mode it would start in. */
  restarting: { id: string; mode: Mode } | null;
  /** The tab whose Pause bar is up: the first-time question, or the "Paused. Continue to type." bar. Never up with a close or restart question. */
  pauseBar: { id: string; kind: "confirm" | "typing" } | null;
  /** The tab whose name is being edited. */
  renaming: string | null;
  /** Bumped when focus should go back to the terminal. */
  focusToken: number;
  /**
   * The tabs with a resume request out. The core announces Running before the new session exists, so a terminal for such a tab
   * must not attach yet: the window holds it back until the response is in.
   */
  resuming: string[];
  /** Per tab, how many times a resume has come back ok. The window keys the tab's terminal on it, so a resumed tab gets a fresh attach. */
  resumed: Record<string, number>;
};

type Api = Pick<typeof sessions, "close" | "reopen" | "rename" | "reorder" | "restart" | "resume" | "pause" | "cont">;
type Context = {
  endpoint: Endpoint | null;
  infos: SessionInfo[];
  selectedId: string | undefined;
  /** False in the rail and focus layouts, which have no row to edit. Left out means expanded. */
  sidebarExpanded?: boolean;
  /** The sidebar's rows while it is grouped by project (left out otherwise): moves follow their displayed order. */
  groupedRows?: RowModel[];
};
type Deps = {
  api: Api;
  context: () => Context;
  select: (tabId: string) => void;
  /** The last measured terminal size, which a resumed session starts at. Left out means the default. */
  size?: () => { cols: number; rows: number };
  /** The settings writer's view of the first-Pause warning. Left out means it was never shown and nothing is saved. */
  settings?: { readonly pauseWarned: boolean; markPauseWarned(): Promise<void> };
};

/** The size a resumed session starts at until a terminal has been measured. */
const DEFAULT_SIZE = { cols: 120, rows: 30 };

const reason = (e: unknown): string => (e instanceof ApiError ? e.message : String(e));

/**
 * What the person does to the session list from the keyboard or the sidebar: close (with the busy-agent bar), reopen, rename,
 * reorder, go to a session. It sends the request and shows a toast on a refusal; the session list itself only changes with the
 * core's next snapshot, so nothing here is optimistic. Plain class so it can be tested without a DOM; LiveApp wraps it.
 */
export class SessionActions {
  private state: ActionsState = { toast: null, closing: null, restarting: null, pauseBar: null, renaming: null, focusToken: 0, resuming: [], resumed: {} };
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

  /** Hands the focus back to the focused session's terminal (after a sheet that typed into it closes). */
  focusTerminal(): void {
    this.set({ focusToken: this.refocus() });
  }

  /** Shows a receipt as a toast (and the window's announcer says it): a merge or a removal that finished in the Review sheet. */
  notify(text: string): void {
    this.showToast(text);
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
        const { infos, selectedId, groupedRows } = this.deps.context();
        // Grouped, the list the person sees is the groups one after another, and a move stays inside the session's project.
        const next = selectedId === undefined ? null : groupedRows ? moveWithinGroup(groupedRows, selectedId, shortcut.by) : moveBy(infos.map((i) => i.tab_id), selectedId, shortcut.by);
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
      case "review":
      case "cheatsheet":
        break;
    }
  }

  /** Ctrl+Shift+W on the selected session (or the row menu's Close, on `tabId`): closes it, or puts the bar up when an agent may be mid-task. */
  async requestClose(tabId?: string): Promise<void> {
    const { infos, selectedId } = this.deps.context();
    const info = infos.find((i) => i.tab_id === (tabId ?? selectedId));
    // While the bar is up, the key does nothing: closing must stay a deliberate press on "Close now".
    if (!info || this.state.closing === info.tab_id || this.closingNow.has(info.tab_id)) return;
    if (needsCloseConfirm(info)) this.set({ closing: info.tab_id, pauseBar: null });
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

  /**
   * Resume on an Ended or Exited tab (the button, or the palette). One request per tab at a time: a second press while one is out
   * does nothing, and shows no error. A refusal (the core's 409) is announced in the core's own words; the button keeps its focus,
   * since nothing here moves it. Only a response that says ok frees the tab's terminal to attach (see `resuming`).
   */
  async resume(tabId: string): Promise<void> {
    const { endpoint, infos } = this.deps.context();
    const info = infos.find((i) => i.tab_id === tabId);
    // Gone, or already running: there is nothing to start. (The core would refuse a running one too, but that is no news to show.)
    if (!endpoint || !info || info.state.kind === "running" || this.state.resuming.includes(tabId)) return;
    this.set({ resuming: [...this.state.resuming, tabId] });
    let text: string;
    let ok = false;
    try {
      const result = await this.deps.api.resume(endpoint, tabId, this.deps.size?.() ?? DEFAULT_SIZE);
      ok = result.ok;
      text = result.ok ? "Resumed" : result.reason;
    } catch (e) {
      text = `Couldn't resume the session: ${reason(e)}`;
    }
    // One change frees the tab and, when it worked, moves the key of its terminal: the terminal mounts after the response, never before.
    const resuming = this.state.resuming.filter((id) => id !== tabId);
    this.set(ok ? { resuming, resumed: { ...this.state.resumed, [tabId]: (this.state.resumed[tabId] ?? 0) + 1 } } : { resuming });
    this.showToast(text);
  }

  /**
   * Mode ▸ (the row menu, the header chip, or a palette Mode command): puts the restart bar up for `id` in `mode`. It does nothing,
   * and says false, for a plain shell (it has no mode), an unknown tab, and the mode the session is already in; the caller then
   * keeps the focus where it is. One question at a time: a new one replaces the one up.
   */
  askRestart(id: string, mode: Mode): boolean {
    const info = this.deps.context().infos.find((i) => i.tab_id === id);
    if (!info || info.agent === null || info.mode === mode) return false;
    this.set({ restarting: { id, mode }, pauseBar: null });
    return true;
  }

  /** Cancel or Escape on the restart bar: nothing is sent, and focus goes back to the session's terminal. */
  cancelRestart(): void {
    if (this.state.restarting !== null) this.set({ restarting: null, focusToken: this.refocus() });
  }

  /**
   * "Restart" on the bar. The bar goes away at once, and focus goes to the terminal whether or not it works. On success the tab's
   * `resumed` count moves, so the window gives it a new terminal, attached after the response; a refusal (the core's 409) shows
   * its own words in the toast and changes nothing.
   */
  async confirmRestart(): Promise<void> {
    const ask = this.state.restarting;
    const { endpoint } = this.deps.context();
    if (!ask) return;
    this.set({ restarting: null, focusToken: this.refocus() });
    if (!endpoint) return;
    try {
      const result = await this.deps.api.restart(endpoint, ask.id, ask.mode, this.deps.size?.() ?? DEFAULT_SIZE);
      if (result.ok) this.set({ resumed: { ...this.state.resumed, [ask.id]: (this.state.resumed[ask.id] ?? 0) + 1 }, focusToken: this.refocus() });
      else this.showToast(result.reason);
    } catch (e) {
      this.showToast(`Couldn't restart the session: ${reason(e)}`);
    }
  }

  /**
   * Pause (the row menu or the palette). The first Pause ever puts the confirm bar up and says "asked"; after that it sends at once
   * and says "sent". Null for a session that is not running, already paused, or unknown: nothing happens. One question at a time:
   * the bar replaces a close or restart question.
   */
  pause(id: string): "asked" | "sent" | null {
    const info = this.deps.context().infos.find((i) => i.tab_id === id);
    if (!info || info.state.kind !== "running" || info.paused) return null;
    if (this.deps.settings?.pauseWarned !== true) {
      this.set({ pauseBar: { id, kind: "confirm" }, closing: null, restarting: null });
      return "asked";
    }
    void this.sendPause(id);
    return "sent";
  }

  /** "Pause" on the confirm bar: the warning is remembered first, then the request goes. A settings write that fails does not stop it; the person confirmed. */
  async confirmPause(): Promise<void> {
    const ask = this.state.pauseBar;
    if (ask?.kind !== "confirm") return;
    this.set({ pauseBar: null, focusToken: this.refocus() });
    try {
      await this.deps.settings?.markPauseWarned();
    } catch {
      // Asked again next time; nothing to tell the person now.
    }
    await this.sendPause(ask.id);
  }

  private async sendPause(id: string): Promise<void> {
    const { endpoint } = this.deps.context();
    if (!endpoint) return;
    try {
      const result = await this.deps.api.pause(endpoint, id);
      if (!result.ok) this.showToast(`Couldn't pause the session: ${result.reason}`);
    } catch (e) {
      this.showToast(`Couldn't pause the session: ${reason(e)}`);
    }
  }

  /** Continue (the row menu, the palette, or the typing bar). Nothing happens for a session that is not paused. The row follows the core's next snapshot. */
  async cont(id: string): Promise<void> {
    const { endpoint, infos } = this.deps.context();
    const info = infos.find((i) => i.tab_id === id);
    if (!endpoint || !info || !info.paused) return;
    try {
      const result = await this.deps.api.cont(endpoint, id);
      if (!result.ok) this.showToast(`Couldn't continue the session: ${result.reason}`);
    } catch (e) {
      this.showToast(`Couldn't continue the session: ${reason(e)}`);
    }
  }

  /**
   * Keys were typed into paused session `id`: puts the "Paused. Continue to type." bar up. A close or restart question that is
   * already up stays; a dismissed bar comes back with the next key.
   */
  showPausedBar(id: string): void {
    const info = this.deps.context().infos.find((i) => i.tab_id === id);
    const bar = this.state.pauseBar;
    if (!info || !info.paused || this.state.closing !== null || this.state.restarting !== null) return;
    if (bar?.id === id && bar.kind === "typing") return;
    this.set({ pauseBar: { id, kind: "typing" } });
  }

  /** "Continue" on the typing bar. The bar stays until the snapshot says the session is running (LiveApp then cancels it, which hands the focus back). */
  async continueFromBar(): Promise<void> {
    const bar = this.state.pauseBar;
    if (bar?.kind === "typing") await this.cont(bar.id);
  }

  /** Cancel, Dismiss or Escape on either Pause bar: nothing is sent, and focus goes back to the terminal. */
  cancelPause(): void {
    if (this.state.pauseBar !== null) this.set({ pauseBar: null, focusToken: this.refocus() });
  }

  /** F2 on the selected session (or the row menu's Rename, on `tabId`). It does nothing where the sidebar is not expanded: there is no row to turn into a text box. */
  startRename(tabId?: string): void {
    const { infos, selectedId, sidebarExpanded = true } = this.deps.context();
    const id = tabId ?? selectedId;
    if (sidebarExpanded && infos.some((i) => i.tab_id === id)) this.set({ renaming: id ?? null });
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
