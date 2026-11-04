import type { DoctorPhase } from "./DoctorPage";
import { initialSelection } from "./report";
import type { DoctorFixPlan, DoctorLastFix, DoctorReport } from "./types";

/** Everything the live Doctor page keeps: where a check or fix is, what is checked, the sheet's plan and the last fix. */
export type DoctorModel = {
  phase: DoctorPhase;
  report: DoctorReport | null;
  selected: ReadonlySet<string>;
  /** The fix sheet's plan; the sheet is open while it is set. */
  plan: DoctorFixPlan | null;
  /** A file changed after the plan was made: the sheet offers Reload only. */
  conflict: boolean;
  undo: DoctorLastFix | null;
  failure?: string;
};

export const INITIAL_DOCTOR: DoctorModel = { phase: "running", report: null, selected: new Set(), plan: null, conflict: false, undo: null };

export type DoctorAction =
  | { type: "runStarted" }
  | { type: "runFailed"; message: string }
  | { type: "reportLoaded"; report: DoctorReport }
  | { type: "toggle"; id: string }
  | { type: "planned"; plan: DoctorFixPlan }
  | { type: "cancelPlan" }
  | { type: "fixStarted" }
  | { type: "fixApplied"; fix: DoctorLastFix }
  | { type: "fixConflict" }
  | { type: "fixFailed" }
  | { type: "undoLoaded"; fix: DoctorLastFix | null }
  | { type: "undoStale" }
  | { type: "undone" };

/**
 * A report that arrives during a fix, or after it, is not shown: the rows the fix ticked off stay on screen until Run again.
 * The selection is the person's own across reports (`initialSelection` keeps the state of a row that was already listed).
 */
export function doctorReducer(s: DoctorModel, a: DoctorAction): DoctorModel {
  switch (a.type) {
    case "runStarted":
      return { ...s, phase: "running", plan: null, conflict: false, failure: undefined };
    case "runFailed":
      return { ...s, phase: "failed", failure: a.message };
    case "reportLoaded":
      if (s.phase === "fixing" || s.phase === "fixed") return s;
      return { ...s, phase: "ready", report: a.report, selected: initialSelection(a.report, s.report ? s.selected : undefined, s.report ?? undefined), failure: undefined };
    case "toggle": {
      const selected = new Set(s.selected);
      if (!selected.delete(a.id)) selected.add(a.id);
      return { ...s, selected };
    }
    case "planned":
      return { ...s, plan: a.plan, conflict: false };
    case "cancelPlan":
      return { ...s, plan: null, conflict: false };
    case "fixStarted":
      return { ...s, phase: "fixing" };
    case "fixApplied":
      return { ...s, phase: "fixed", plan: null, conflict: false, undo: a.fix };
    case "fixConflict":
      return { ...s, phase: "ready", conflict: true };
    case "fixFailed":
      return { ...s, phase: "ready", plan: null, conflict: false };
    case "undoLoaded":
      return { ...s, undo: a.fix };
    case "undoStale":
      return s.undo ? { ...s, undo: { ...s.undo, files_match: false } } : s;
    case "undone":
      return { ...s, undo: null, phase: "running" };
  }
}
