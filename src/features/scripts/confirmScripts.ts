import type { RepoScripts } from "../../generated/RepoScripts";
import type { SessionInfo } from "../../generated/SessionInfo";
import { sessionRunning } from "../sessions/resumeState";

/** What a task's confirm view says about the repo's `marshell.json`. Each variant carries the file as it was read. */
export type ConfirmScripts =
  | { kind: "ask"; scripts: RepoScripts; allowLabel: string; cancelLabel: string; /** The task already exists: only the card shows, with no commands and no way back. */ only?: boolean }
  | { kind: "setup"; scripts: RepoScripts; script: string }
  | { kind: "problem"; scripts: RepoScripts; text: string };

/** The scripts a file holds, in the order the card lists them. */
export const scriptLines = (s: Pick<RepoScripts, "setup" | "run" | "teardown">): string[] => [
  ...(s.setup === null ? [] : [`setup: ${s.setup}`]),
  ...(s.run === null ? [] : [`run: ${s.run}`]),
  ...(s.teardown === null ? [] : [`teardown: ${s.teardown}`]),
];

/**
 * What the confirm view shows for the main checkout's file. A file that couldn't be read, or holds nothing to run, shows nothing.
 * A problem is one line, and the task is made without scripts.
 */
export function confirmScripts(scripts: RepoScripts | null): ConfirmScripts | undefined {
  if (scripts === null) return undefined;
  if (scripts.problem !== null) return { kind: "problem", scripts, text: scripts.problem };
  if (scriptLines(scripts).length === 0) return undefined;
  if (!scripts.trusted) return { kind: "ask", scripts, allowLabel: "Allow and create", cancelLabel: "Create without scripts" };
  return scripts.setup === null ? undefined : { kind: "setup", scripts, script: scripts.setup };
}

/** The same path, ignoring case and the kind of slash (Windows paths). */
const norm = (p: string) => p.replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();

/** The header's (and the palette's) script control: Run, or Stop when this repo's run script is going in a tab. */
export type RunControl = { kind: "run"; script: string } | { kind: "stop"; script: string; tabId: string };

/**
 * `scripts` is the selected session's repo. Stop is the running session with `script === "run"` whose folder is the repo's root.
 * Run needs a run script and a file with no problem.
 */
export function runControl(scripts: RepoScripts | null, infos: SessionInfo[]): RunControl | null {
  if (scripts === null) return null;
  const running = infos.find((i) => i.script === "run" && sessionRunning(i) && norm(i.cwd) === norm(scripts.root));
  if (running) return { kind: "stop", script: scripts.run ?? "", tabId: running.tab_id };
  if (scripts.problem !== null || scripts.run === null) return null;
  return { kind: "run", script: scripts.run };
}
