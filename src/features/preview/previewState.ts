import { useCallback, useReducer } from "react";

/** One file in the preview's history. `line` and `col` are 1-based and come from a clicked path such as `src/a.rs:42:7`. */
export type PreviewEntry = { path: string; line?: number; col?: number; view: "rendered" | "source" };

export type PreviewState = { open: boolean; entries: PreviewEntry[]; index: number };

export type PreviewAction =
  | { type: "open"; entry: PreviewEntry }
  | { type: "back" }
  | { type: "forward" }
  | { type: "close" }
  | { type: "toggle" }
  | { type: "setView"; view: PreviewEntry["view"] };

export const INITIAL: PreviewState = { open: false, entries: [], index: -1 };

export const currentOf = (s: PreviewState): PreviewEntry | undefined => s.entries[s.index];

export function previewReducer(s: PreviewState, a: PreviewAction): PreviewState {
  switch (a.type) {
    case "open": {
      // A new file ends the forward history, as in a browser.
      const entries = [...s.entries.slice(0, s.index + 1), a.entry];
      return { open: true, entries, index: entries.length - 1 };
    }
    case "back":
      return s.index > 0 ? { ...s, index: s.index - 1 } : s;
    case "forward":
      return s.index < s.entries.length - 1 ? { ...s, index: s.index + 1 } : s;
    case "close":
      return s.open ? { ...s, open: false } : s;
    case "toggle":
      // With no history there is nothing to reopen; the caller opens the palette instead.
      return s.entries.length === 0 ? s : { ...s, open: !s.open };
    case "setView":
      return s.index < 0 ? s : { ...s, entries: s.entries.map((e, i) => (i === s.index ? { ...e, view: a.view } : e)) };
  }
}

export type PreviewControls = {
  state: PreviewState;
  open: (entry: Omit<PreviewEntry, "view"> & { view?: PreviewEntry["view"] }) => void;
  close: () => void;
  toggle: () => void;
  back: () => void;
  forward: () => void;
  setView: (view: PreviewEntry["view"]) => void;
};

/** The preview's state and its actions, for LiveApp and, through `openPreview`, for clicked paths and the palette. */
export function usePreview(): PreviewControls {
  const [state, dispatch] = useReducer(previewReducer, INITIAL);
  return {
    state,
    open: useCallback((entry) => dispatch({ type: "open", entry: { view: "rendered", ...entry } }), []),
    close: useCallback(() => dispatch({ type: "close" }), []),
    toggle: useCallback(() => dispatch({ type: "toggle" }), []),
    back: useCallback(() => dispatch({ type: "back" }), []),
    forward: useCallback(() => dispatch({ type: "forward" }), []),
    setView: useCallback((view) => dispatch({ type: "setView", view }), []),
  };
}
