import { useEffect, useId, useMemo, useReducer, useRef, type Dispatch, type KeyboardEvent } from "react";
import { DiffView } from "../../components/DiffView/DiffView";
import { EmptyState } from "../../components/EmptyState/EmptyState";
import { Page } from "../../components/Page/Page";
import type { Task } from "../../generated/Task";
import type { TaskChanges } from "../../generated/TaskChanges";
import { useDialogFocus } from "../../lib/useDialogFocus";
import { initialReview, reviewReducer, type ReviewAction, type ReviewState } from "./reviewState";
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

/** The step bodies. Only "changes" is here; the others are filled in where the merge and remove sheets are built. */
function ChangesStep({ changes, selected, dispatch }: { changes: TaskChanges; selected: number; dispatch: Dispatch<ReviewAction> }) {
  const base = useId();
  const list = useRef<HTMLDivElement>(null);
  const sections = useMemo(() => splitDiff(changes.diff), [changes.diff]);
  const count = changes.files.length;
  const index = Math.min(selected, count - 1);
  const file = changes.files[index];
  const optionId = (i: number) => `${base}-file-${i}`;

  // The page body scrolls, not the list, so the keyboard's choice is kept in sight by hand.
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

export type ReviewViewProps = {
  task: Task;
  changes: TaskChanges;
  state: ReviewState;
  dispatch: Dispatch<ReviewAction>;
  onMerge?: () => void;
  onRemove?: () => void;
  onClose?: () => void;
};

/** The sheet drawn for a state: pure, so it renders in a test and in Storybook. */
export function ReviewView({ task, changes, state, dispatch, onMerge, onRemove, onClose }: ReviewViewProps) {
  const actions = (
    <>
      <button
        type="button"
        className="settings__btn"
        data-primary
        disabled={changes.files.length === 0}
        onClick={() => {
          dispatch({ type: "open", step: "merge" });
          onMerge?.();
        }}
      >
        Merge back…
      </button>
      <button
        type="button"
        className="settings__btn"
        onClick={() => {
          dispatch({ type: "open", step: "remove" });
          onRemove?.();
        }}
      >
        Remove…
      </button>
      {onClose && (
        <button type="button" className="settings__btn" onClick={onClose}>
          Close
        </button>
      )}
    </>
  );
  return (
    <Page title={`Changes in ${task.branch}`} subtitle={subtitleOf(task, changes)} actions={actions}>
      {state.step === "changes" && <ChangesStep changes={changes} selected={state.selected} dispatch={dispatch} />}
    </Page>
  );
}

export type ReviewSheetProps = {
  task: Task;
  changes: TaskChanges;
  /** The agent's display name, for "Ask <Agent> to fix them" on the conflict step. */
  agentName: string;
  onClose: () => void;
  /** Merge back… was chosen: the caller loads the plan for the merge step. */
  onMerge: () => void;
  /** Remove… was chosen: the caller loads the plan for the remove step. */
  onRemove: () => void;
  /** Types a prompt into the task's agent session (the conflict step). */
  onAskAgent: (text: string) => void;
};

/**
 * Review a task: its changed files and the diff of each, then, in the same sheet, the merge and the removal. It is a modal
 * dialog like Appearance: focus lands on the dialog, Tab stays inside, Esc closes and focus goes back to what opened it.
 */
export function ReviewSheet({ task, changes, onClose, onMerge, onRemove }: ReviewSheetProps) {
  const [state, dispatch] = useReducer(reviewReducer, undefined, initialReview);
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(dialog, dialog, onClose);
  return (
    <div ref={dialog} className="review" role="dialog" aria-modal="true" aria-label={`Changes in ${task.branch}`} tabIndex={-1}>
      <ReviewView task={task} changes={changes} state={state} dispatch={dispatch} onMerge={onMerge} onRemove={onRemove} onClose={onClose} />
    </div>
  );
}
