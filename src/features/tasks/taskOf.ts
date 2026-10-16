import type { SessionInfo } from "../../generated/SessionInfo";
import type { Task } from "../../generated/Task";

/** A Windows path starts with a drive letter or a UNC share; only those compare without regard to case. */
const isWindowsPath = (p: string) => /^[A-Za-z]:/.test(p) || p.startsWith("\\\\");

/**
 * The text two paths are compared by, like the core's `projects::same_path`: separators are all "/", a trailing separator is
 * dropped (a root such as `C:/` or `/` keeps one), and a Windows path ignores ASCII case. The core also resolves short names
 * on disk, which the page cannot do; the cwd of a task's session is the task's own path, so the text matches.
 */
function key(path: string): string {
  const slashed = path.replace(/\\/g, "/");
  let trimmed = slashed.replace(/\/+$/, "");
  if (trimmed === "" || /^[A-Za-z]:$/.test(trimmed)) trimmed = slashed.slice(0, trimmed.length + 1);
  return isWindowsPath(path) ? trimmed.replace(/[A-Z]/g, (c) => c.toLowerCase()) : trimmed;
}

export function samePath(a: string, b: string): boolean {
  return key(a) === key(b);
}

/** The task whose worktree the session runs in, if any: the session's folder is the task's folder (not a folder inside it). */
export function taskOf(info: SessionInfo, tasks: Task[]): Task | undefined {
  return tasks.find((t) => samePath(info.cwd, t.path));
}
