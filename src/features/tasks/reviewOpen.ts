import type { SessionInfo } from "../../generated/SessionInfo";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { findCommand, type KeyEventLike } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import { agentLabel } from "../sessions/sessionActions";
import type { RowModel } from "../sidebar/types";
import { taskOf } from "./taskOf";

/** What the announcer says when Review is asked for on a session that does not run in a task's folder. */
export const NOT_A_TASK = "This session isn't a task.";

/** What the announcer says when the core's task list could not be read, so a real task is never called "not a task". */
export const taskListFailed = (reason: string): string => `Couldn't load the task list. ${reason}`;

/** `open`: review this task. `none`: say `message`. `load`: the task list has not loaded (or failed), so ask for it again and decide then (resolveTarget). */
export type ReviewTarget = { kind: "open"; tabId: string; task: Task } | { kind: "none"; message: string } | { kind: "load"; tabId: string };

/**
 * The task to review for the session in focus, or the sentence to say when there is none. `tasks` is null while the list has not
 * loaded: that is not "not a task", so the answer is to load it.
 */
export function reviewTarget(infos: SessionInfo[], focusedId: string | undefined, tasks: Task[] | null): ReviewTarget {
  const info = infos.find((i) => i.tab_id === focusedId);
  if (!info) return { kind: "none", message: NOT_A_TASK };
  if (tasks === null) return { kind: "load", tabId: info.tab_id };
  const task = taskOf(info, tasks);
  return task ? { kind: "open", tabId: info.tab_id, task } : { kind: "none", message: NOT_A_TASK };
}

/** The core's answer to a task list request: the list, or the reason there is none. */
export type TaskListResult = { kind: "ok"; tasks: Task[] } | { kind: "failed"; reason: string };

/** Loads the list for a `load` target and decides with it; any other target is returned as it is. Failing to load is said as such, never as "not a task". */
export async function resolveTarget(target: ReviewTarget, infos: () => SessionInfo[], reload: () => Promise<TaskListResult>): Promise<Exclude<ReviewTarget, { kind: "load" }>> {
  if (target.kind !== "load") return target;
  const result = await reload();
  if (result.kind === "failed") return { kind: "none", message: taskListFailed(result.reason) };
  // The list was asked for a moment ago; the session in focus is read again, in case it went away meanwhile.
  const next = reviewTarget(infos(), target.tabId, result.tasks);
  return next.kind === "load" ? { kind: "none", message: NOT_A_TASK } : next;
}

/** A text box keeps its own keys. xterm's hidden textarea is the terminal, not a box. Duck-typed, so it needs no DOM to test. */
export function isTextBox(target: { tagName?: string; classList?: { contains(name: string): boolean } } | null): boolean {
  if (!target) return false;
  return (target.tagName === "INPUT" || target.tagName === "TEXTAREA") && !target.classList?.contains("xterm-helper-textarea");
}

/** The key press the window reads for Review: the code and modifiers (lib/bindings), whether it is a held key, and the way to cancel its default. */
export type ReviewKeyEvent = KeyEventLike & { repeat: boolean; preventDefault(): void };

/** Something else holds the keys: the launcher, an overlay, the Review sheet itself, or a text box. */
export function keyBlocked(s: { launcherOpen: boolean; overlay: string | null; review: boolean; inTextBox: boolean }): boolean {
  return s.launcherOpen || s.overlay !== null || s.review || s.inTextBox;
}

/**
 * What a key press does for Review (Ctrl+Shift+R, Cmd+R on macOS), or null when it is another key. The window owns this key
 * everywhere: it is always prevented, so the webview's hard reload never runs, and then acted on only when nothing else holds
 * the keys, it is not a held key, and the session in focus is a task.
 */
export function reviewKeyDown(
  e: ReviewKeyEvent,
  os: Os,
  ctx: { infos: SessionInfo[]; focusedId: string | undefined; tasks: Task[] | null; blocked: boolean },
): ReviewTarget | { kind: "ignored" } | null {
  if (findCommand(e, os)?.id !== "review-task") return null;
  e.preventDefault();
  if (ctx.blocked || e.repeat) return { kind: "ignored" };
  return reviewTarget(ctx.infos, ctx.focusedId, ctx.tasks);
}

/**
 * The rows with the Task tag on those whose session runs in a task's folder. A row that is not a task is the same object, and with
 * no task (or none matching) the same array, so the sidebar does not redraw for it.
 */
export function markTasks(rows: RowModel[], infos: SessionInfo[], tasks: Task[] | null): RowModel[] {
  if (tasks === null || tasks.length === 0) return rows;
  let changed = false;
  const marked = rows.map((r) => {
    const info = infos.find((i) => i.tab_id === r.id);
    if (!info || !taskOf(info, tasks)) return r;
    changed = true;
    return { ...r, task: true };
  });
  return changed ? marked : rows;
}

/** The agent the sheet may ask to fix a conflict: named only while it is a running agent session. */
export function reviewAgentName(info: SessionInfo | undefined): string | null {
  return info && info.agent !== null && info.state.kind === "running" ? agentLabel(info.agent) : null;
}

/** The palette's Mode commands: only an agent session has a mode to restart in, so a plain shell in focus is not offered them. */
const MODE_COMMANDS = ["mode-manual", "mode-plan", "mode-auto-edit", "mode-full-auto", "mode-bypass"];
const HIDDEN = new Map<string, readonly string[]>();

/**
 * The palette commands left out now: Resume unless the session in focus can resume, Review unless it is a task, and Clean up a
 * missing task unless a task's folder is gone, Copy the recording path when no session is in focus, the Mode commands unless the session in focus is an agent, Pause unless it is running, and Continue unless it is paused. The same array for the same answers, so the palette's list is not rebuilt for them.
 */
export function paletteHidden({ resumable, isTask, missing, focused = true, agent = true, pausable = true, paused = true }: { resumable: boolean; isTask: boolean; missing: boolean; focused?: boolean; agent?: boolean; pausable?: boolean; paused?: boolean }): readonly string[] {
  const key = `${resumable}${isTask}${missing}${focused}${agent}${pausable}${paused}`;
  let hidden = HIDDEN.get(key);
  if (!hidden) {
    hidden = [...(resumable ? [] : ["resume"]), ...(isTask ? [] : ["review-task"]), ...(missing ? [] : ["clean-up-task"]), ...(focused ? [] : ["copy-recording-path"]), ...(agent ? [] : MODE_COMMANDS), ...(pausable ? [] : ["pause"]), ...(paused ? [] : ["continue"])];
    HIDDEN.set(key, hidden);
  }
  return hidden;
}

/** The task "Clean up a missing task…" opens: the first one whose folder is gone. After it is removed the next one is offered. */
export function missingTask(tasks: Task[] | null): Task | null {
  return tasks?.find((t) => t.folder_missing) ?? null;
}

const NO_CHANGES: TaskChanges = { files: [], diff: "", truncated: false, ahead: 0, uncommitted: false };

/** The changes the sheet opens with. A task whose folder is gone has none to read; its sheet is only the remove step. */
export function changesFor(task: Task, load: () => Promise<TaskChanges>): Promise<TaskChanges> {
  return task.folder_missing ? Promise.resolve(NO_CHANGES) : load();
}

/**
 * Asks the core for the task list each time it is called (on start, after every create, merge or remove, and when a review is asked
 * for while the list has not loaded). The newest answer wins: a slower, older one that arrives later is not set. When the core does
 * not answer, the list on screen stays, and the call says why (the next call tries again).
 */
export function taskRefresher(list: () => Promise<Task[]>, set: (tasks: Task[]) => void): () => Promise<TaskListResult> {
  let latest = 0;
  return async () => {
    const mine = ++latest;
    try {
      const tasks = await list();
      if (mine === latest) set(tasks);
      return { kind: "ok", tasks };
    } catch (e) {
      console.warn("Could not load the task list", e);
      return { kind: "failed", reason: e instanceof Error ? e.message : String(e) };
    }
  };
}
