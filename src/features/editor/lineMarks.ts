import { RangeSet, StateEffect, StateField, type Extension } from "@codemirror/state";
import { gutter, GutterMarker } from "@codemirror/view";

/**
 * Sets the marked lines: 1-based line numbers in the document as it is after the transaction that carries the effect. A reload
 * sends the new text and the lines that changed in the same transaction.
 */
export const setMarks = StateEffect.define<number[]>();

class Mark extends GutterMarker {
  toDOM(): HTMLElement {
    const el = document.createElement("span");
    el.className = "editor__mark";
    return el;
  }
}
const mark = new Mark();

/** The marks stay put while the text does not change, and the first transaction that changes the document clears them. */
const marks = StateField.define<RangeSet<GutterMarker>>({
  create: () => RangeSet.empty,
  update(set, tr) {
    for (const e of tr.effects)
      if (e.is(setMarks)) {
        const doc = tr.newDoc;
        const lines = [...new Set(e.value)].filter((n) => Number.isInteger(n) && n >= 1 && n <= doc.lines).sort((a, b) => a - b);
        return RangeSet.of(lines.map((n) => mark.range(doc.line(n).from)));
      }
    return tr.docChanged ? RangeSet.empty : set;
  },
});

/** A gutter with a mark on each numbered line that changed on disk. It holds its width when no line is marked, so the text does not move. */
export function lineMarks(): Extension {
  return [
    marks,
    gutter({
      class: "editor__marks",
      markers: (view) => view.state.field(marks),
      initialSpacer: () => mark,
    }),
  ];
}
