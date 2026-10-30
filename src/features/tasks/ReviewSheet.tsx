import { useEffect, useEffectEvent, useId, useMemo, useReducer, useRef, useState, type ChangeEvent, type Dispatch, type KeyboardEvent } from "react";
import { DiffView } from "../../components/DiffView/DiffView";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { useDialogFocus } from "../../lib/useDialogFocus";
import type { MergePlan } from "../../generated/MergePlan";
import type { RemovePlan } from "../../generated/RemovePlan";
import { CommandList } from "./CommandList";
import {
  PLAN_DELAY_MS,
  REMOVE_KEY,
  canConfirm,
  conflictHeading,
  conflictPrompt,
  confirmMergeRequest,
  confirmRemoveRequest,
  currentLoad,
  initialRun,
  isStale,
  mergePlanRequest,
  mergedToast,
  planFailure,
  planKey,
  readableMessage,
  removeLabel,
  removedToast,
  runEdited,
  runFailed,
  runMerge,
  runOpened,
  runRemove,
  type MergeView,
  type PlanLoad,
  type RemoveView,
  type ReviewApi,
} from "./reviewRun";
import { canClose, initialReview, isBusy, reviewReducer, stepFocus, type ReviewAction, type ReviewState } from "./reviewState";
import { sectionNote, splitDiff } from "./splitDiff";
import "../settings/Settings.css";
import "./ReviewSheet.css";

/** What each status the core reports is called. A status this page does not know is shown as the core wrote it. */
const STATUS_WORDS: Record<string, string> = { added: "Added", modified: "Modified", deleted: "Deleted", renamed: "Renamed", untracked: "New, not committed" };
export const statusWord = (status: string) => STATUS_WORDS[status] ?? status;

const LIST_KEYS = ["ArrowDown", "ArrowUp", "Home", "End"];

/** "Against main · 2 commits ahead · uncommitted changes", leaving out what is zero or false. */
function subtitleOf(task: Task, changes: TaskChanges): string {
  const parts = [`Against ${task.base_branch}`];
  if (changes.ahead > 0) parts.push(`${changes.ahead} ${changes.ahead === 1 ? "commit" : "commits"} ahead`);
  if (changes.uncommitted) parts.push("uncommitted changes");
  return parts.join(" · ");
}

/** The right side of the changes step: the selected file's diff, or one sentence on why there is none to draw. */
function FileDiff({ path, section, label }: { path: string; section: string | undefined; label: string }) {
  // A section with no hunk (a rename with no edits, an empty new file) has nothing for the diff view to draw.
  const note = section === undefined ? "none" : (sectionNote(section) ?? (/^@@/m.test(section) ? null : "none"));
  if (section !== undefined && note === null) return <DiffView diff={section} label={label} expanded />;
  return (
    <div className="review__note">
      <p className="review__note-path" dir="auto">
        {path}
      </p>
      <p className="review__note-text">
        {note === "binary" ? "This is a binary file, so there is nothing to preview." : note === "large" ? "This file is too large to preview." : "There is no diff to show for this file."}
      </p>
    </div>
  );
}

/** The changes step: the files beside the diff of the one chosen. */
function ChangesStep({ changes, selected, dispatch }: { changes: TaskChanges; selected: number; dispatch: Dispatch<ReviewAction> }) {
  const base = useId();
  const list = useRef<HTMLDivElement>(null);
  const sections = useMemo(() => splitDiff(changes.diff), [changes.diff]);
  const count = changes.files.length;
  const index = Math.min(selected, count - 1);
  const file = changes.files[index];
  const optionId = (i: number) => `${base}-file-${i}`;

  // The list scrolls inside its own cap, so the keyboard's choice is kept in sight by hand.
  useEffect(() => {
    list.current?.querySelector(`[id="${base}-file-${index}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [base, index]);

  if (!file) return <EmptyState title="No changes yet." body="Work in this task's folder shows up here once there is something to merge." />;
  const onKeyDown = (e: KeyboardEvent) => {
    if (!LIST_KEYS.includes(e.key)) return;
    e.preventDefault();
    dispatch({ type: "move", key: e.key, count });
  };
  return (
    <div className="review__changes">
      {changes.truncated && <p className="review__banner">The diff is too large to show in full.</p>}
      <div className="review__split">
        <div ref={list} className="review__files" role="listbox" aria-label="Changed files" aria-activedescendant={optionId(index)} tabIndex={0} onKeyDown={onKeyDown}>
          {changes.files.map((f, i) => (
            <div key={f.path} id={optionId(i)} role="option" aria-selected={i === index} className="review__file" data-selected={i === index || undefined} onClick={() => dispatch({ type: "select", index: i, count })}>
              <span className="review__status">{statusWord(f.status)}</span>
              <span className="review__path" dir="auto">
                {f.path}
              </span>
            </div>
          ))}
        </div>
        <div className="review__diff">
          <FileDiff path={file.path} section={sections.find((s) => s.path === file.path)?.diff} label={`Changes to ${file.path}`} />
        </div>
      </div>
    </div>
  );
}

/** The one status line while the core works ("Merging…"). It is a polite live region, so it is read out once, when it appears. */
function Progress({ text }: { text: string }) {
  return (
    <p className="review__progress" role="status">
      {text}
    </p>
  );
}

/**
 * The commands, dimmed and marked while they are not the current plan (the message or the checkbox changed, or the task may have).
 * The words above them are a polite status line: it stays mounted through a burst of typing, so it is said once, not per key.
 */
function Commands({ commands, teardown, stale }: { commands: string[]; teardown?: string; stale: boolean }) {
  return (
    <div className="review__commands" data-stale={stale || undefined}>
      {stale && (
        <p className="review__stale" role="status">
          Updating the commands…
        </p>
      )}
      <CommandList commands={commands} teardown={teardown} />
    </div>
  );
}

/** What went wrong, in the core's words. A bar on the start edge says "problem" as well as the colour does. */
function Failure({ text }: { text: string }) {
  return (
    <p className="review__error" role="alert">
      {text}
    </p>
  );
}

/** What would be closed or lost, as a list above the commands. Nothing when there is nothing to say. */
function Warnings({ warnings }: { warnings: string[] }) {
  if (warnings.length === 0) return null;
  return (
    <ul className="review__warnings" aria-label="Warnings">
      {warnings.map((w, i) => (
        <li key={i}>{w}</li>
      ))}
    </ul>
  );
}

/** The merge step: the commit message, the checkbox, and the commands the merge will run. The commands shown are the ones that run. */
function MergeStep({ merge, busy }: { merge: MergeView; busy: boolean }) {
  const id = useId();
  return (
    <div className="review__step">
      <div className="field">
        <label className="field__label" htmlFor={id}>
          Commit message
        </label>
        <input
          id={id}
          type="text"
          className="field__control review__message"
          spellCheck={false}
          value={merge.message}
          readOnly={busy}
          onChange={(e: ChangeEvent<HTMLInputElement>) => merge.onMessage(e.target.value)}
        />
      </div>
      <label className="review__check">
        <input type="checkbox" checked={merge.remove} disabled={busy} onChange={(e: ChangeEvent<HTMLInputElement>) => merge.onRemoveChange(e.target.checked)} />
        <span>Remove the worktree and branch afterwards</span>
      </label>
      {merge.error && <Failure text={merge.error} />}
      {merge.plan?.blocker && <p className="review__blocker">{merge.plan.blocker}</p>}
      <Warnings warnings={merge.plan?.warnings ?? []} />
      {merge.plan && (
        <>
          <p className="review__lede">Marshell will run these commands.</p>
          <Commands commands={merge.plan.commands} teardown={merge.plan.teardown} stale={!busy && isStale(merge.plan, merge.ready)} />
        </>
      )}
      {busy && <Progress text="Merging…" />}
    </div>
  );
}

/** The remove step: what would be lost, then the commands that remove the folder and the branch. */
function RemoveStep({ remove, busy, folderMissing }: { remove: RemoveView; busy: boolean; folderMissing: boolean }) {
  const warnings = remove.plan?.warnings ?? [];
  return (
    <div className="review__step">
      {folderMissing && <p className="review__lede">The task folder is gone, so there is nothing to review or merge.</p>}
      {remove.error && <Failure text={remove.error} />}
      <Warnings warnings={warnings} />
      {remove.plan && (
        <>
          <p className="review__lede">Marshell will run these commands.</p>
          <Commands commands={remove.plan.commands} teardown={remove.plan.teardown} stale={!busy && isStale(remove.plan, remove.ready)} />
        </>
      )}
      {busy && <Progress text="Removing…" />}
    </div>
  );
}

/** The conflict step: what stopped the merge, and the files to look at. */
function ConflictStep({ task, files }: { task: Task; files: string[] }) {
  return (
    <div className="review__step">
      <h3 className="review__heading">{conflictHeading(files.length, task.base_branch)}</h3>
      <ul className="review__conflicts" aria-label="Files that conflict">
        {files.map((f) => (
          <li key={f} className="review__conflict" dir="auto">
            {f}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Nothing running, nothing loaded: what the view draws when it is given no merge or remove to show. */
const NO_MERGE: MergeView = { message: "", remove: true, plan: null, ready: false, error: null, onMessage: () => {}, onRemoveChange: () => {}, onConfirm: () => {} };
const NO_REMOVE: RemoveView = { plan: null, ready: false, error: null, onConfirm: () => {} };

export type ReviewViewProps = {
  task: Task;
  changes: TaskChanges;
  state: ReviewState;
  dispatch: Dispatch<ReviewAction>;
  /** The merge step's message, checkbox, plan and confirm. */
  merge?: MergeView;
  /** The remove step's plan and confirm. */
  remove?: RemoveView;
  /** The files the merge could not combine (the conflict step). */
  conflictFiles?: string[];
  /** The display name of the task's running agent, or null when it has none: "Ask <name> to fix them" is shown only with a name. */
  agentName?: string | null;
  /** Gets the conflict prompt into the agent's prompt line; the sheet closes after it. */
  onAskAgent?: (text: string) => void;
  onClose?: () => void;
};

/**
 * The sheet drawn for a state: pure, so it renders in a test and in Storybook. Each step has its own buttons; while the core
 * works (merging, removing) they stay where they are but do nothing, and the sheet cannot be closed.
 */
export function ReviewView({ task, changes, state, dispatch, merge = NO_MERGE, remove = NO_REMOVE, conflictFiles = [], agentName = null, onAskAgent, onClose }: ReviewViewProps) {
  const { step } = state;
  const busy = isBusy(step);
  // aria-disabled, never `disabled`: the button that was pressed keeps focus while the core works or the commands load.
  const canMerge = canConfirm({ step, ready: merge.ready, blocker: merge.plan?.blocker ?? null, running: false });
  const canRemove = canConfirm({ step, ready: remove.ready, blocker: null, running: false });
  const back = (
    <button type="button" className="settings__btn" aria-disabled={busy || undefined} onClick={() => !busy && dispatch({ type: "back" })}>
      Back
    </button>
  );
  let actions: React.ReactNode = null;
  let title = `Changes in ${task.branch}`;
  let subtitle: string | undefined = subtitleOf(task, changes);
  let body: React.ReactNode = null;
  if (step === "changes") {
    actions = (
      <>
        <button type="button" className="settings__btn" data-primary disabled={changes.files.length === 0} onClick={() => dispatch({ type: "open", step: "merge" })}>
          Merge back…
        </button>
        <button type="button" className="settings__btn" onClick={() => dispatch({ type: "open", step: "remove" })}>
          Remove…
        </button>
        {onClose && (
          <button type="button" className="settings__btn" onClick={onClose}>
            Close
          </button>
        )}
      </>
    );
    body = <ChangesStep changes={changes} selected={state.selected} dispatch={dispatch} />;
  } else if (step === "merge" || step === "merging") {
    title = `Merge ${task.branch} into ${task.base_branch}`;
    subtitle = undefined;
    actions = (
      <>
        <button type="button" className="settings__btn review__confirm" data-primary aria-disabled={!canMerge || undefined} onClick={() => canMerge && merge.onConfirm()}>
          Merge
        </button>
        {back}
      </>
    );
    body = <MergeStep merge={merge} busy={busy} />;
  } else if (step === "remove" || step === "removing") {
    title = `Remove ${task.branch}`;
    subtitle = undefined;
    actions = (
      <>
        <button type="button" className="settings__btn review__confirm" data-primary aria-disabled={!canRemove || undefined} onClick={() => canRemove && remove.onConfirm()}>
          {removeLabel(remove.plan?.warnings ?? [])}
        </button>
        {/* With the folder gone the sheet is only this step, so there is nothing to go back to. */}
        {task.folder_missing
          ? onClose && (
              <button type="button" className="settings__btn" onClick={onClose}>
                Close
              </button>
            )
          : back}
      </>
    );
    body = <RemoveStep remove={remove} busy={busy} folderMissing={task.folder_missing} />;
  } else if (step === "conflict") {
    title = `Merge ${task.branch} into ${task.base_branch}`;
    subtitle = undefined;
    actions = (
      <>
        {agentName !== null && (
          <button
            type="button"
            className="settings__btn review__confirm"
            data-primary
            onClick={() => {
              onAskAgent?.(conflictPrompt(task, conflictFiles));
              onClose?.();
            }}
          >
            {`Ask ${agentName} to fix them`}
          </button>
        )}
        <button type="button" className={agentName === null ? "settings__btn review__confirm" : "settings__btn"} onClick={onClose}>
          Close
        </button>
      </>
    );
    body = <ConflictStep task={task} files={conflictFiles} />;
  }
  return (
    <Page title={title} subtitle={subtitle} actions={actions}>
      {body}
    </Page>
  );
}

export type ReviewSheetProps = {
  task: Task;
  changes: TaskChanges;
  /** The display name of the task's running agent session, or null when it has none (a shell, or an agent that has exited). */
  agentName: string | null;
  /** The core, for this task. The sheet reads it when it asks, so a changed `api` is used from the next request on; it does not make the sheet ask again. */
  api: ReviewApi;
  /** Closes the sheet. Called by Close and Esc, never while the core is merging or removing. */
  onClose: () => void;
  /** A merge or a removal finished: the receipt to show as a toast ("Merged marshell/fix-login into main."). The sheet closes right after. */
  onDone: (message: string) => void;
  /** Types a prompt into the task's agent session, without Enter. The sheet closes right after; the caller moves focus to that terminal. */
  onAskAgent: (text: string) => void;
};

/**
 * Review a task: its changed files and the diff of each, then, in the same sheet, the merge and the removal. It is a modal
 * dialog like Appearance: focus lands on the dialog, Tab stays inside, Esc closes and focus goes back to what opened it.
 *
 * Shown is run. A merge or removal is sent with the commands (and warnings) of the plan on screen, and only while that plan is
 * the current one: Merge and Remove are aria-disabled while a refetch is waiting or out. The core refuses if the task changed
 * after the plan was drawn; that reason is shown and the plan is asked for again.
 */
export function ReviewSheet({ task, changes, agentName, api, onClose, onDone, onAskAgent }: ReviewSheetProps) {
  const [state, dispatch] = useReducer(reviewReducer, task.folder_missing, initialReview);
  const { step } = state;
  const busy = isBusy(step);
  const dialog = useRef<HTMLDivElement>(null);
  // Esc and Close do nothing while the core works.
  const close = canClose(step) ? onClose : undefined;
  useDialogFocus(dialog, dialog, close);

  const [message, setMessage] = useState(() => readableMessage(task.name));
  const [removeAfter, setRemoveAfter] = useState(true);
  /** The round of asking for plans (bumped when a step opens and when a run fails, so every plan on screen is stale) and why the last run was refused. */
  const [{ round, failure }, setRun] = useState(initialRun);
  const [mergeLoad, setMergeLoad] = useState<PlanLoad<MergePlan> | null>(null);
  const [removeLoad, setRemoveLoad] = useState<PlanLoad<RemovePlan> | null>(null);
  const [conflicts, setConflicts] = useState<string[]>([]);
  const typed = useRef(false);
  const running = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const send = (action: ReviewAction) => {
    if (action.type === "open") setRun(runOpened);
    else if (action.type === "back" || action.type === "start") setRun(runEdited);
    dispatch(action);
  };

  // The commands for the message and checkbox as they stand. Typing waits for a pause (PLAN_DELAY_MS); opening the step does not.
  const key = planKey(message, removeAfter);
  const askMergePlan = useEffectEvent((m: string, r: boolean) => api.mergePlan(mergePlanRequest(m, r)));
  useEffect(() => {
    if (step !== "merge") return;
    let live = true;
    const wait = typed.current ? PLAN_DELAY_MS : 0;
    typed.current = false;
    const timer = setTimeout(() => {
      askMergePlan(message, removeAfter).then(
        (plan) => live && setMergeLoad({ key, round, plan, error: null }),
        (e) => live && setMergeLoad({ key, round, plan: null, error: planFailure(e) }),
      );
    }, wait);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [step, key, message, removeAfter, round]);

  const askRemovePlan = useEffectEvent(() => api.removePlan());
  useEffect(() => {
    if (step !== "remove") return;
    let live = true;
    askRemovePlan().then(
      (plan) => live && setRemoveLoad({ key: REMOVE_KEY, round, plan, error: null }),
      (e) => live && setRemoveLoad({ key: REMOVE_KEY, round, plan: null, error: planFailure(e) }),
    );
    return () => {
      live = false;
    };
  }, [step, round]);

  // Focus goes to the first thing to do in a step; see stepFocus.
  const lastStep = useRef(step);
  useEffect(() => {
    const target = stepFocus(lastStep.current, step);
    lastStep.current = step;
    if (target) dialog.current?.querySelector<HTMLElement>(target)?.focus();
  }, [step]);

  const currentMerge = currentLoad(mergeLoad, key, round);
  const currentRemove = currentLoad(removeLoad, REMOVE_KEY, round);

  const confirmMerge = () => {
    const req = confirmMergeRequest({ step, running: running.current, message, remove: removeAfter, key, round, load: mergeLoad });
    if (!req) return;
    running.current = true;
    send({ type: "start" });
    void runMerge(api, req).then((outcome) => {
      running.current = false;
      if (!mounted.current) return;
      if (outcome.kind === "merged") {
        dispatch({ type: "finish" });
        onDone(mergedToast(task, outcome.removed, outcome.removal_note, outcome.notes));
        onClose();
      } else if (outcome.kind === "conflict") {
        setConflicts(outcome.files);
        dispatch({ type: "conflict" });
      } else {
        // Back on the merge step with the reason; the plan is asked for again, because the task may have changed.
        setRun((s) => runFailed(s, outcome.reason));
        dispatch({ type: "fail" });
      }
    });
  };

  const confirmRemove = () => {
    const req = confirmRemoveRequest({ step, running: running.current, round, load: removeLoad });
    if (!req) return;
    running.current = true;
    send({ type: "start" });
    void runRemove(api, req).then((result) => {
      running.current = false;
      if (!mounted.current) return;
      if (result.ok) {
        dispatch({ type: "finish" });
        onDone(removedToast(task, result.notes));
        onClose();
      } else {
        setRun((s) => runFailed(s, result.reason));
        dispatch({ type: "fail" });
      }
    });
  };

  const merge: MergeView = {
    message,
    remove: removeAfter,
    // A plan for an earlier message stays up, dimmed and marked as updating (see Commands), until the new one arrives.
    plan: (currentMerge ?? mergeLoad)?.plan ?? null,
    ready: currentMerge?.plan != null,
    error: failure ?? currentMerge?.error ?? null,
    onMessage: (m) => {
      typed.current = true;
      setRun(runEdited);
      setMessage(m);
    },
    onRemoveChange: (r) => {
      typed.current = true;
      setRun(runEdited);
      setRemoveAfter(r);
    },
    onConfirm: confirmMerge,
  };
  const remove: RemoveView = {
    plan: (currentRemove ?? removeLoad)?.plan ?? null,
    ready: currentRemove?.plan != null,
    error: failure ?? currentRemove?.error ?? null,
    onConfirm: confirmRemove,
  };

  return (
    <div ref={dialog} className="review" role="dialog" aria-modal="true" aria-label={task.folder_missing ? `Remove ${task.branch}` : `Changes in ${task.branch}`} tabIndex={-1}>
      <ReviewView
        task={task}
        changes={changes}
        state={state}
        dispatch={send}
        merge={merge}
        remove={remove}
        conflictFiles={conflicts}
        agentName={agentName}
        onAskAgent={onAskAgent}
        onClose={close}
      />
    </div>
  );
}
