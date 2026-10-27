import type { SettingsPlan } from "../../generated/SettingsPlan";
import type { ApplyOutcome } from "../../lib/api/agents";
import type { CompanyReview } from "./CompanySection";

/** What a plan or undo request came back with; a failure keeps the HTTP status and the core's sentence. */
export type PlanResult = { ok: true; plan: SettingsPlan } | { ok: false; status: number; message: string };
export type UndoResult = { ok: true } | { ok: false; status: number; message: string };

export const conflictLine = (path: string) => `${path} changed since you opened it. Reload to see it.`;
export const undoneLine = (path: string) => `Put ${path} back as it was.`;
const undoConflictLine = (path: string) => `${path} changed since this change, so undo would overwrite it.`;

export const afterPlan = (r: PlanResult): CompanyReview => (r.ok ? { kind: "review", plan: r.plan } : r.status === 409 ? { kind: "conflict" } : { kind: "error", message: r.message });

export const afterApply = (o: ApplyOutcome): CompanyReview => (o.ok ? { kind: "saved", snapshotId: o.result.snapshot_id } : o.conflict ? { kind: "conflict" } : { kind: "error", message: o.message });

/** Undo puts the file back, so the form is editable again; a 409 means the file moved on since. */
export const afterUndo = (r: UndoResult, path: string): CompanyReview => (r.ok ? { kind: "editing" } : r.status === 409 ? { kind: "error", message: undoConflictLine(path) } : { kind: "error", message: r.message });

/** A draft edit leaves a finished result behind; a busy state is left alone. */
export const afterEdit = (review: CompanyReview): CompanyReview => (review.kind === "saved" || review.kind === "conflict" || review.kind === "error" ? { kind: "editing" } : review);

/** Planning and applying take no new request. */
export const isBusy = (review: CompanyReview) => review.kind === "planning" || review.kind === "applying";

/**
 * Where focus goes when the flow moves from `prev` to `next`, since the focused button has gone: the review or the result line,
 * so a held Enter can't land on Apply or Undo, or Review changes on the way back. Null leaves it to the page's own fallback.
 */
export const focusAfter = (prev: CompanyReview["kind"], next: CompanyReview["kind"]): "review" | "result" | "review-button" | null => {
  if (next === "review") return "review";
  if (next === "saved" || next === "conflict") return "result";
  if (prev === "review" && next === "editing") return "review-button";
  return null;
};

/** What `announce` says when the flow lands on a state; null when it says nothing. */
export const announcement = (review: CompanyReview, _name: string, path: string): string | null => {
  switch (review.kind) {
    case "review":
      return `Review the change to ${path}.`;
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
