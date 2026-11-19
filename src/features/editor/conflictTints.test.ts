import { EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import { conflictKeys, conflictTints, jumpToConflict, tintsOf } from "./conflictTints";
import { COLOUR_CAP } from "./tokens";

const TEXT = ["a", "<<<<<<< mine", "x", "=======", "y", ">>>>>>> disk", "z"].join("\n");
const stateOf = (doc: string) => EditorState.create({ doc, extensions: [conflictTints()] });
const classOf = (doc: string, line: number) => tintsOf(stateOf(doc)).find((t) => t.line === line)?.classes;

describe("conflictTints", () => {
  it("tints the three kinds of line", () => {
    expect(classOf(TEXT, 1)).toBeUndefined();
    expect(classOf(TEXT, 2)).toBe("editor__conflict editor__conflict--mark");
    expect(classOf(TEXT, 3)).toBe("editor__conflict editor__conflict--mine");
    expect(classOf(TEXT, 4)).toBe("editor__conflict editor__conflict--mark");
    expect(classOf(TEXT, 5)).toBe("editor__conflict editor__conflict--disk");
    expect(classOf(TEXT, 6)).toBe("editor__conflict editor__conflict--mark");
    expect(classOf(TEXT, 7)).toBeUndefined();
  });

  it("tints a base section", () => {
    const text = ["<<<<<<< mine", "m", "||||||| base", "b", "=======", "d", ">>>>>>> disk"].join("\n");
    expect([1, 2, 3, 4, 5, 6, 7].map((n) => classOf(text, n)?.replace("editor__conflict ", ""))).toEqual([
      "editor__conflict--mark",
      "editor__conflict--mine",
      "editor__conflict--mark",
      "editor__conflict--base",
      "editor__conflict--mark",
      "editor__conflict--disk",
      "editor__conflict--mark",
    ]);
  });

  it("follows an edit", () => {
    const s = stateOf(TEXT);
    const middle = s.doc.line(4);
    const next = s.update({ changes: { from: middle.from, to: middle.to + 1 } }).state;
    expect(tintsOf(s)).not.toEqual([]);
    expect(tintsOf(next)).toEqual([]);
  });

  it("has none above the colour cap", () => {
    const doc = TEXT + "\n" + "x".repeat(COLOUR_CAP);
    expect(doc.length).toBeGreaterThan(COLOUR_CAP);
    expect(tintsOf(stateOf(doc))).toEqual([]);
  });

  it("conflictKeys binds F7 and Shift-F7", () => {
    const keys = conflictKeys(() => {});
    expect(keys.map((k) => k.key)).toEqual(["F7", "Shift-F7"]);
    expect(keys.every((k) => k.preventDefault === true && typeof k.run === "function")).toBe(true);
  });
});

describe("jumpToConflict", () => {
  const TWO = ["<<<<<<< mine", "x", "=======", "y", ">>>>>>> disk", "between", "<<<<<<< mine", "p", "=======", "q", ">>>>>>> disk", "after"].join("\n");
  /** A view with no DOM: it holds a state, takes a dispatch's selection and counts what else was asked of it. */
  const viewOf = (doc: string, line: number) => {
    let state = EditorState.create({ doc });
    state = state.update({ selection: { anchor: state.doc.line(line).from } }).state;
    const seen = { dispatches: 0, focused: 0 };
    const view = {
      get state() {
        return state;
      },
      scrollDOM: { clientHeight: 300 },
      dispatch(spec: { selection?: { anchor: number } }) {
        seen.dispatches += 1;
        state = state.update({ selection: spec.selection }).state;
      },
      focus() {
        seen.focused += 1;
      },
    };
    const cursor = () => state.doc.lineAt(state.selection.main.head).number;
    return { view: view as unknown as EditorView, seen, cursor };
  };

  it("goes to the next conflict from the cursor and wraps to the first", () => {
    const v = viewOf(TWO, 6);
    expect(jumpToConflict(v.view, "next")).toEqual({ at: 2, of: 2 });
    expect(v.cursor()).toBe(7);
    expect(jumpToConflict(v.view, "next")).toEqual({ at: 1, of: 2 });
    expect(v.cursor()).toBe(1);
  });
  it("goes to the previous conflict and wraps to the last", () => {
    const v = viewOf(TWO, 6);
    expect(jumpToConflict(v.view, "previous")).toEqual({ at: 1, of: 2 });
    expect(v.cursor()).toBe(1);
    expect(jumpToConflict(v.view, "previous")).toEqual({ at: 2, of: 2 });
    expect(v.cursor()).toBe(7);
  });
  it("goes to the first from anywhere", () => {
    const v = viewOf(TWO, 12);
    expect(jumpToConflict(v.view, "first")).toEqual({ at: 1, of: 2 });
    expect(v.cursor()).toBe(1);
  });
  it("leaves the cursor alone when the text has no conflict", () => {
    const v = viewOf("a\nb\nc", 2);
    expect(jumpToConflict(v.view, "next")).toEqual({ at: 0, of: 0 });
    expect(v.seen.dispatches).toBe(0);
    expect(v.cursor()).toBe(2);
  });
  it("does not take the keyboard, because a merge's answer can come while the person is somewhere else", () => {
    const v = viewOf(TWO, 6);
    jumpToConflict(v.view, "first");
    expect(v.seen.focused).toBe(0);
  });
});
