import { Annotation, Compartment, EditorSelection, EditorState, Transaction, type TransactionSpec } from "@codemirror/state";
import { isolateHistory } from "@codemirror/commands";
import { EditorView, keymap } from "@codemirror/view";
import { indentUnit } from "@codemirror/language";
import { gotoLine } from "@codemirror/search";
import { useEffect, useImperativeHandle, useMemo, useRef, type Ref } from "react";
import { conflictKeys, conflictTints, jumpToConflict } from "./conflictTints";
import { EDITOR_HINT } from "./EditorFooter";
import { languageFor } from "./fileTypes";
import { lineMarks, setMarks } from "./lineMarks";
import { EDITOR_DEFAULTS, type EditorPrefs } from "./editorPrefs";
import { access, extensions, INDENT, indentOf, reconfigurePrefs, READ_ONLY, toggleWrap, WRAP, wrapping } from "./setup";
import { shiki } from "./shikiBridge";
import "./Editor.css";

/** What the pane can ask of an editor. */
export type EditorHandle = {
  /** The text as it is now. */
  text(): string;
  /** Replaces the whole text in one step that undo can take back, and marks the lines (1-based) that changed. Not an edit by the person: `onEdit` does not fire. */
  setText(text: string, marks: number[]): void;
  focus(): void;
  /** Opens the go-to-line box. */
  goToLine(): void;
  toggleWrap(): void;
  /** Scrolls a line (1-based) to about a third from the top. */
  reveal(line: number): void;
  /** Puts the cursor on a conflict: the first, or the next or previous one from the cursor. */
  toConflict(which: "first" | "next" | "previous"): void;
};

type Props = {
  name: string;
  path: string;
  /** The text the editor opens with. Later text comes through `setText`. */
  initial: string;
  /** The file's first line, for a type that its name does not tell (a shebang). */
  firstLine?: string;
  readOnly: boolean;
  wrap: boolean;
  /** The Files settings the editor reads; a change reconfigures the open view. Absent: the defaults. */
  prefs?: EditorPrefs;
  /** A line to scroll to a third from the top; scrolls again when it changes. */
  target?: number;
  /** The text after a change the person made: typing, paste, cut, undo, redo, replace. */
  onEdit: (text: string) => void;
  /** The cursor's line and column, both from 1. */
  onCursor: (line: number, column: number) => void;
  /** Whether a key press is one of the app's shortcuts: the editor leaves such keys out, so they reach the window. Absent: none is. */
  appChord?: (e: KeyboardEvent) => boolean;
  /** The id of the footer's hint, which describes the text box. */
  hintId?: string;
  /** Where a jump to a conflict landed: its place (from 1) and the count, or 0 and 0 when the text has none. */
  onConflict?: (at: number, of: number) => void;
  ref?: Ref<EditorHandle>;
};

/** Marks a transaction as `setText`'s, which is not an edit by the person. */
const FROM_OUTSIDE = Annotation.define<boolean>();

/**
 * The change that replaces the whole text with one that came from outside the editor (a reload, a merge, Take disk), with the
 * lines to mark. The cursor stays on its line number, and in its column where the line is long enough.
 */
export function outsideText(state: EditorState, text: string, marks: number[]): TransactionSpec {
  const { doc, selection } = state;
  const head = selection.main.head;
  const at = doc.lineAt(head);
  const column = head - at.from;
  const next = EditorState.create({ doc: text }).doc;
  const line = next.line(Math.min(at.number, next.lines));
  const cursor = Math.min(line.from + column, line.to);
  return {
    changes: { from: 0, to: doc.length, insert: text },
    selection: EditorSelection.cursor(cursor),
    effects: setMarks.of(marks),
    // A step of its own: the history would otherwise join it to typing within half a second on either side, and an undo
    // would then pass the text from before a merge, which is the text that brings the bar back.
    annotations: [FROM_OUTSIDE.of(true), Transaction.addToHistory.of(true), isolateHistory.of("full")],
  };
}
/** The Shiki language, a compartment of its own so a renamed file is coloured for its new type. */
const COLOURS = new Compartment();

const never = () => false;

/**
 * A CodeMirror 6 editor. The element is drawn on the server pass and in tests without a DOM; the view itself is made in an
 * effect. The props that later change (name, read-only, wrap, language) reconfigure the view; the text only changes through
 * the person or `setText`.
 */
export function Editor({ name, path, initial, firstLine, readOnly, wrap, prefs = EDITOR_DEFAULTS, target, onEdit, onCursor, appChord, hintId = EDITOR_HINT, onConflict, ref }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  // The callbacks as of the last render, so the view made once never calls an old one.
  const calls = useRef({ onEdit, onCursor, onConflict });
  calls.current = { onEdit, onCursor, onConflict };
  const language = useMemo(() => languageFor(path, firstLine), [path, firstLine]);
  // What the view was last given, so an unchanged prop does not reconfigure it.
  const given = useRef({ name, readOnly, wrap, language, prefs });

  useEffect(() => {
    const parent = host.current;
    if (!parent) return;
    const v = new EditorView({
      parent,
      state: EditorState.create({
        doc: initial,
        extensions: [
          extensions({ readOnly, wrap, name, appChord: appChord ?? never, prefs }),
          COLOURS.of(shiki(language)),
          // Tab puts in what the file is indented with, so a tab-indented file does not get spaces; a file with no indent yet gets
          // the tab size in spaces.
          INDENT.of(indentUnit.of(indentOf(initial, language, prefs.tab_size))),
          lineMarks(),
          conflictTints(),
          keymap.of(conflictKeys((at, of) => calls.current.onConflict?.(at, of))),
          EditorView.contentAttributes.of({ "aria-describedby": hintId }),
          EditorView.updateListener.of((u) => {
            // A change by the person: not one of setText's.
            if (u.transactions.some((t) => t.docChanged && !t.annotation(FROM_OUTSIDE))) calls.current.onEdit(u.state.doc.toString());
            if (u.selectionSet || u.docChanged) report(u.state);
          }),
        ],
      }),
    });
    const report = (state: EditorState) => {
      const head = state.selection.main.head;
      const line = state.doc.lineAt(head);
      calls.current.onCursor(line.number, head - line.from + 1);
    };
    view.current = v;
    given.current = { name, readOnly, wrap, language, prefs };
    report(v.state);
    return () => {
      v.destroy();
      view.current = null;
    };
    // The view is made once; what changes later is applied by the effects below.
  }, []);

  useEffect(() => {
    const v = view.current;
    const was = given.current;
    if (!v) return;
    if (was.name !== name || was.readOnly !== readOnly) v.dispatch({ effects: READ_ONLY.reconfigure(access(name, readOnly)) });
    if (was.wrap !== wrap) v.dispatch({ effects: WRAP.reconfigure(wrapping(wrap)) });
    if (was.language !== language) v.dispatch({ effects: COLOURS.reconfigure(shiki(language)) });
    // The settings come as a new object on each change, and the same values in a new object are no change.
    const p = was.prefs;
    if (p.tab_indents !== prefs.tab_indents || p.indent_guides !== prefs.indent_guides || p.whitespace !== prefs.whitespace || p.scroll_past_end !== prefs.scroll_past_end || p.tab_size !== prefs.tab_size) {
      reconfigurePrefs(v, prefs, appChord ?? never);
      // The Tab key's indent follows the tab size where the file has no indent of its own.
      if (p.tab_size !== prefs.tab_size) v.dispatch({ effects: INDENT.reconfigure(indentUnit.of(indentOf(v.state.doc.toString(), language, prefs.tab_size))) });
    }
    given.current = { name, readOnly, wrap, language, prefs };
  }, [name, readOnly, wrap, language, prefs, appChord]);

  const reveal = (line: number) => {
    const v = view.current;
    if (!v) return;
    const at = v.state.doc.line(Math.min(Math.max(1, Math.floor(line)), v.state.doc.lines)).from;
    // Aligned to the top with a margin of a third of the height, so the line sits about a third from the top, as the read-only view puts it.
    v.dispatch({ effects: EditorView.scrollIntoView(at, { y: "start", yMargin: v.scrollDOM.clientHeight / 3 }) });
  };

  useEffect(() => {
    if (target !== undefined) reveal(target);
  }, [target]);

  useImperativeHandle(
    ref,
    () => ({
      text: () => view.current?.state.doc.toString() ?? "",
      setText(text, marks) {
        const v = view.current;
        if (!v) return;
        if (v.state.doc.toString() === text) {
          v.dispatch({ effects: setMarks.of(marks) });
          return;
        }
        v.dispatch(outsideText(v.state, text, marks));
      },
      focus: () => view.current?.focus(),
      goToLine() {
        const v = view.current;
        if (!v) return;
        v.focus();
        gotoLine(v);
      },
      toggleWrap() {
        const v = view.current;
        if (v) toggleWrap(v);
      },
      reveal,
      toConflict(which) {
        const v = view.current;
        if (!v) return;
        const r = jumpToConflict(v, which);
        // "first" follows a merge's answer, which can come while the person is somewhere else: it says nothing (the merge's
        // own line is being read) and leaves the keyboard where it is. The other two are a command the person just ran.
        if (which === "first") return;
        v.focus();
        calls.current.onConflict?.(r.at, r.of);
      },
    }),
    [],
  );

  return (
    <div className="editor">
      <div className="editor__host" ref={host} data-hint={hintId} />
    </div>
  );
}
