import { getIndentUnit } from "@codemirror/language";
import type { Extension, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

/** The columns of a line's leading spaces and tabs, and whether anything but white space follows. */
function leading(line: string, tabSize: number): { columns: number; blank: boolean } {
  let columns = 0;
  for (const ch of line) {
    if (ch === " ") columns += 1;
    else if (ch === "\t") columns += tabSize - (columns % tabSize);
    else return { columns, blank: false };
  }
  return { columns, blank: true };
}

/**
 * How many guides each line draws: its leading white space in columns (a tab runs to the next multiple of `tabSize`) over `unit`,
 * whole levels only. A blank line takes the smaller of the first lines with text above and below it, and nothing when it has
 * none on a side, so a guide runs through a gap inside a block and stops at its end.
 */
export function guideLevels(lines: readonly string[], tabSize: number, unit: number): number[] {
  const step = Math.max(1, unit);
  const read = lines.map((l) => leading(l, tabSize));
  const levels = read.map((r) => (r.blank ? 0 : Math.floor(r.columns / step)));
  const above: number[] = [];
  let last = 0;
  for (let i = 0; i < read.length; i++) {
    if (!read[i]!.blank) last = levels[i]!;
    above.push(last);
  }
  let next = 0;
  for (let i = read.length - 1; i >= 0; i--) {
    if (read[i]!.blank) levels[i] = Math.min(above[i]!, next);
    else next = levels[i]!;
  }
  return levels;
}

/** How far past the visible lines the plugin looks for the text a blank line at the edge lies between. */
const REACH = 100;

function build(view: EditorView): DecorationSet {
  const { state } = view;
  const { doc } = state;
  const tabSize = state.tabSize;
  const unit = Math.max(1, getIndentUnit(state));
  const ranges: Array<{ from: number; to: number }> = [];
  for (const r of view.visibleRanges) ranges.push({ from: doc.lineAt(r.from).number, to: doc.lineAt(r.to).number });
  const marks: Range<Decoration>[] = [];
  let done = 0;
  for (const r of ranges) {
    // A range that starts inside the one before it has been drawn.
    const first = Math.max(r.from, done + 1);
    if (first > r.to) continue;
    const from = Math.max(1, first - REACH);
    const to = Math.min(doc.lines, r.to + REACH);
    const texts: string[] = [];
    for (let n = from; n <= to; n++) texts.push(doc.line(n).text);
    const levels = guideLevels(texts, tabSize, unit);
    for (let n = first; n <= r.to; n++) {
      const level = levels[n - from]!;
      if (level > 0) marks.push(Decoration.line({ attributes: { class: "editor__guided", style: `--guides: ${level}; --guide-unit: ${unit}` } }).range(doc.line(n).from));
    }
    done = r.to;
  }
  return Decoration.set(marks);
}

const guides = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = build(view);
    }
    update(u: ViewUpdate) {
      const unitChanged = getIndentUnit(u.startState) !== getIndentUnit(u.state) || u.startState.tabSize !== u.state.tabSize;
      if (u.docChanged || u.viewportChanged || unitChanged) this.decorations = build(u.view);
    }
  },
  { decorations: (v) => v.decorations },
);

/**
 * Indent guides: a line decoration on each visible line that is indented, `--guides` the number of levels and `--guide-unit` the
 * columns in one, which Editor.css draws as thin lines one level apart. The unit is the file's own indent (`indentOf`), so a file
 * indented with two spaces has a guide every two columns.
 */
export function indentGuides(): Extension {
  return guides;
}
