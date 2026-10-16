import { stepActive } from "../launcher/launcherKeys";

/**
 * Where the Review sheet is. The changes step is a list of files and a diff; "merge" and "remove" show the commands before
 * they run; "merging" and "removing" wait for the core; "conflict" lists the files a merge could not combine; "done" says what
 * happened. The steps after "changes" have their bodies in the merge-and-remove task; the transitions are all here.
 */
export type ReviewStep = "changes" | "merge" | "merging" | "conflict" | "remove" | "removing" | "done";

export type ReviewState = {
  step: ReviewStep;
  /** The index of the file whose diff shows. Kept across steps, so Back lands on the file that was open. */
  selected: number;
};

export type ReviewAction =
  /** An arrow, Home or End in the file list; `count` is how many files there are. */
  | { type: "move"; key: string; count: number }
  | { type: "select"; index: number; count: number }
  /** Merge back… or Remove…: from the changes step only. */
  | { type: "open"; step: "merge" | "remove" }
  /** Back: from a step that is waiting for the user, to the changes. Not while the core is working. */
  | { type: "back" }
  /** The user confirmed: merge goes to merging, remove to removing. */
  | { type: "start" }
  /** The core said it is done. */
  | { type: "finish" }
  /** The merge stopped on conflicts. */
  | { type: "conflict" }
  /** The core refused or git failed: back to the step that asked, to try again. */
  | { type: "fail" };

export const initialReview = (): ReviewState => ({ step: "changes", selected: 0 });

export function reviewReducer(state: ReviewState, action: ReviewAction): ReviewState {
  const { step } = state;
  switch (action.type) {
    case "move": {
      if (step !== "changes" || action.count <= 0) return state;
      if (action.key === "Home") return { ...state, selected: 0 };
      if (action.key === "End") return { ...state, selected: action.count - 1 };
      if (action.key !== "ArrowDown" && action.key !== "ArrowUp") return state;
      return { ...state, selected: stepActive(state.selected, action.key, action.count) };
    }
    case "select":
      return step === "changes" && action.index >= 0 && action.index < action.count ? { ...state, selected: action.index } : state;
    case "open":
      return step === "changes" ? { ...state, step: action.step } : state;
    case "back":
      return step === "merge" || step === "remove" || step === "conflict" ? { ...state, step: "changes" } : state;
    case "start":
      return step === "merge" ? { ...state, step: "merging" } : step === "remove" ? { ...state, step: "removing" } : state;
    case "finish":
      return step === "merging" || step === "removing" ? { ...state, step: "done" } : state;
    case "conflict":
      return step === "merging" ? { ...state, step: "conflict" } : state;
    case "fail":
      return step === "merging" ? { ...state, step: "merge" } : step === "removing" ? { ...state, step: "remove" } : state;
  }
}
