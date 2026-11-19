import { StateField, type EditorState, type Extension } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet, type KeyBinding } from "@codemirror/view";
import { conflictFrom, conflictsIn } from "./conflicts";
import { COLOUR_CAP } from "./tokens";

type Tint = { line: number; classes: string };
type Held = { tints: Tint[]; set: DecorationSet };

const BASE = "editor__conflict";

/** The class of each line of each conflict, in line order. */
function tintsFor(state: EditorState): Tint[] {
  if (state.doc.length > COLOUR_CAP) return [];
  const out: Tint[] = [];
  for (const c of conflictsIn(state.doc.iterLines())) {
    const mark = `${BASE} ${BASE}--mark`;
    const part = (from: number, to: number, kind: string) => {
      for (let n = from + 1; n < to; n += 1) out.push({ line: n, classes: `${BASE} ${BASE}--${kind}` });
    };
    out.push({ line: c.start, classes: mark });
    part(c.start, c.base ?? c.middle, "mine");
    if (c.base !== null) {
      out.push({ line: c.base, classes: mark });
      part(c.base, c.middle, "base");
    }
    out.push({ line: c.middle, classes: mark });
    part(c.middle, c.end, "disk");
    out.push({ line: c.end, classes: mark });
  }
  return out;
}

function held(state: EditorState): Held {
  const tints = tintsFor(state);
  const set = Decoration.set(tints.map((t) => Decoration.line({ class: t.classes }).range(state.doc.line(t.line).from)));
  return { tints, set };
}

const field = StateField.define<Held>({
  create: held,
  update: (value, tr) => (tr.docChanged ? held(tr.state) : value),
  provide: (f) => EditorView.decorations.from(f, (v) => v.set),
});

/** Tints the lines of every conflict: the person's side, the disk's side, a base section and the marker lines. Nothing above `COLOUR_CAP`. */
export function conflictTints(): Extension {
  return field;
}

/** The tinted lines (1-based) and their classes, as the field holds them. */
export function tintsOf(state: EditorState): Tint[] {
  return state.field(field).tints;
}

/**
 * Puts the cursor at the start of a conflict's first line and scrolls it to about a third from the top. It scans the whole
 * text, so it also works above `COLOUR_CAP`, where nothing is tinted. Returns the conflict's 1-based place and the count;
 * with none the cursor stays and the result is `{ at: 0, of: 0 }`. The keyboard is the caller's to move.
 */
export function jumpToConflict(view: EditorView, which: "first" | "next" | "previous"): { at: number; of: number } {
  const { doc, selection } = view.state;
  const conflicts = conflictsIn(doc.iterLines());
  const i = conflictFrom(conflicts, doc.lineAt(selection.main.head).number, which);
  if (i === null) return { at: 0, of: 0 };
  const from = doc.line(conflicts[i]!.start).from;
  view.dispatch({
    selection: { anchor: from },
    effects: EditorView.scrollIntoView(from, { y: "start", yMargin: view.scrollDOM.clientHeight / 3 }),
    userEvent: "select",
  });
  return { at: i + 1, of: conflicts.length };
}

/** F7 goes to the next conflict and Shift+F7 to the previous one. The key is always taken, so it never falls through. */
export function conflictKeys(report: (at: number, of: number) => void): KeyBinding[] {
  const go = (which: "next" | "previous") => (view: EditorView) => {
    const r = jumpToConflict(view, which);
    report(r.at, r.of);
    return true;
  };
  return [
    { key: "F7", run: go("next"), preventDefault: true },
    { key: "Shift-F7", run: go("previous"), preventDefault: true },
  ];
}
