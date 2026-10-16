import type { SessionInfo } from "../../generated/SessionInfo";
import type { Task } from "../../generated/Task";
import { findCommand, type KeyEventLike } from "../../lib/bindings";
import type { Os } from "../../lib/keymap";
import { agentLabel } from "../sessions/sessionActions";
import type { RowModel } from "../sidebar/types";
import { taskOf } from "./taskOf";

/** What the announcer says when Review is asked for on a session that does not run in a task's folder. */
export const NOT_A_TASK = "This session isn't a task.";

export type ReviewTarget = { kind: "open"; tabId: string; task: Task } | { kind: "none"; message: string };

/** The task to review for the session in focus, or the sentence to say when there is none. */
export function reviewTarget(infos: SessionInfo[], focusedId: string | undefined, tasks: Task[]): ReviewTarget {
  const info = infos.find((i) => i.tab_id === focusedId);
  const task = info && taskOf(info, tasks);
  return info && task ? { kind: "open", tabId: info.tab_id, task } : { kind: "none", message: NOT_A_TASK };
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
  ctx: { infos: SessionInfo[]; focusedId: string | undefined; tasks: Task[]; blocked: boolean },
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
export function markTasks(rows: RowModel[], infos: SessionInfo[], tasks: Task[]): RowModel[] {
  if (tasks.length === 0) return rows;
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

const HIDE_NONE: readonly string[] = [];
const HIDE_RESUME: readonly string[] = ["resume"];
const HIDE_REVIEW: readonly string[] = ["review-task"];
const HIDE_BOTH: readonly string[] = ["resume", "review-task"];

/** The palette commands left out now: Resume unless the session in focus can resume, Review unless it is a task. Constants, so the palette's list is not rebuilt for them. */
export function paletteHidden({ resumable, isTask }: { resumable: boolean; isTask: boolean }): readonly string[] {
  if (resumable) return isTask ? HIDE_NONE : HIDE_REVIEW;
  return isTask ? HIDE_RESUME : HIDE_BOTH;
}

/**
 * Asks the core for the task list each time it is called (on start, and after every create, merge or remove). The newest answer wins:
 * a slower, older one that arrives later is dropped. When the core does not answer, the list on screen stays.
 */
export function taskRefresher(list: () => Promise<Task[]>, set: (tasks: Task[]) => void): () => Promise<void> {
  let latest = 0;
  return async () => {
    const mine = ++latest;
    try {
      const tasks = await list();
      if (mine === latest) set(tasks);
    } catch (e) {
      console.warn("Could not load the task list", e);
    }
  };
}
