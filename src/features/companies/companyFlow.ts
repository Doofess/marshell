import type { AgentSettingsView } from "../../generated/AgentSettingsView";
import type { SettingsPlan } from "../../generated/SettingsPlan";
import type { ApplyOutcome } from "../../lib/api/agents";
import type { Draft } from "./companyEdit";
import type { CompanyMode, CompanyReview } from "./CompanySection";

/** What a plan or undo request came back with; a failure keeps the HTTP status and the core's sentence. */
export type PlanResult = { ok: true; plan: SettingsPlan } | { ok: false; status: number; message: string };
export type UndoResult = { ok: true } | { ok: false; status: number; message: string };

export const conflictLine = (path: string) => `${path} changed since you opened it. Reload to see it.`;
export const undoneLine = (path: string) => `Put ${path} back as it was.`;
export const rawDroppedLine = (path: string) => `${path} changed, so your raw edit was dropped. The text is the file as it is now.`;
const undoConflictLine = (path: string) => `${path} changed since this change, so undo would overwrite it.`;

export const afterPlan = (r: PlanResult): CompanyReview => (r.ok ? { kind: "review", plan: r.plan } : r.status === 409 ? { kind: "conflict" } : { kind: "error", message: r.message });

export const afterApply = (o: ApplyOutcome): CompanyReview => (o.ok ? { kind: "saved", snapshotId: o.result.snapshot_id } : o.conflict ? { kind: "conflict" } : { kind: "error", message: o.message });

/** Undo puts the file back, so the form is editable again; a 409 means the file moved on since. */
export const afterUndo = (r: UndoResult, path: string): CompanyReview => (r.ok ? { kind: "undone" } : r.status === 409 ? { kind: "error", message: undoConflictLine(path) } : { kind: "error", message: r.message });

/** A draft edit leaves a finished result behind; a busy state is left alone. */
export const afterEdit = (review: CompanyReview): CompanyReview => (review.kind === "saved" || review.kind === "undone" || review.kind === "conflict" || review.kind === "error" ? { kind: "editing" } : review);

/** Planning, applying and undoing take no new request. */
export const isBusy = (review: CompanyReview) => review.kind === "planning" || review.kind === "applying" || review.kind === "undoing";

/** The whole text a Review sends: only for the raw file shown, when it is editable as text and differs from the file. */
export const rawChange = (mode: CompanyMode, view: AgentSettingsView | null, rawDraft: string | null): string | null =>
  mode === "raw" && view !== null && view.editable && view.raw !== null && view.text !== null && view.raw_lock === null && rawDraft !== null && rawDraft !== view.text ? rawDraft : null;

/**
 * The drafts Reload keeps. A form draft is per key, so it stays and the review shows any key the file changed. A raw
 * draft replaces the whole file, so it is dropped when the file's text moved on, rather than overwrite what changed (PLAN §6).
 */
export const afterReload = (prevText: string | null, next: AgentSettingsView, draft: Draft, rawDraft: string | null): { draft: Draft; rawDraft: string | null; rawDropped: boolean } =>
  rawDraft !== null && next.text !== prevText ? { draft, rawDraft: null, rawDropped: true } : { draft, rawDraft, rawDropped: false };

/**
 * Where focus goes when the flow moves from `prev` to `next`, since the focused button has gone: the review or the result line,
 * so a held Enter can't land on Apply or Undo, or Review changes on the way back. Null leaves it to the page's own fallback.
 */
export const focusAfter = (prev: CompanyReview["kind"], next: CompanyReview["kind"]): "review" | "result" | "review-button" | null => {
  if (next === "review") return "review";
  if (next === "saved" || next === "undone" || next === "conflict" || next === "error") return "result";
  if (prev === "review" && next === "editing") return "review-button";
  return null;
};

export const fieldFocus = (key: string) => `field:${key}`;

/** The control to focus after Discard (the last one edited) or Reload (the last one focused); null when there was none. */
export const focusBack = (action: "discard" | "reload", lastEdited: string | null, lastFocused: string | null): string | null => (action === "discard" ? lastEdited : lastFocused);

/** Focuses the `data-focus` target inside `root`. A target that went away (now locked, or the view fell back to Form) leaves focus to the Settings fallback. */
export const focusIn = (root: { querySelector(selector: string): { focus(): void } | null } | null, key: string): void => {
  root?.querySelector(`[data-focus="${key}"]`)?.focus();
};

/** What `announce` says when the flow lands on a state; null when it says nothing. */
export const announcement = (review: CompanyReview, _name: string, path: string): string | null => {
  switch (review.kind) {
    case "review":
      return `Review the change to ${path}.`;
    case "undone":
      return undoneLine(path);
    case "saved":
      return `Saved ${path}.`;
    case "conflict":
      return conflictLine(path);
    case "error":
      return review.message;
    default:
      return null;
  }
};
