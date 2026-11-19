import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { history, redo, undo } from "@codemirror/commands";
import { EditorState, type Transaction } from "@codemirror/state";
import type { Encoding, Eol } from "./textFile";
import { Editor, outsideText } from "./Editor";
import { EditorFooter } from "./EditorFooter";

describe("outsideText", () => {
  /** A state with the editor's history, and the commands run on it as the keys would. */
  function editing(doc: string) {
    let state = EditorState.create({ doc, extensions: [history()] });
    const dispatch = (tr: Transaction) => void (state = tr.state);
    return {
      text: () => state.doc.toString(),
      type: (text: string) => dispatch(state.update({ changes: { from: state.doc.length, insert: text }, userEvent: "input.type" })),
      set: (text: string) => dispatch(state.update(outsideText(state, text, []))),
      undo: () => undo({ state, dispatch }),
      redo: () => redo({ state, dispatch }),
    };
  }

  it("is_an_undo_step_of_its_own_right_after_typing", () => {
    // A merge answered within half a second of the last key: Ctrl+Z must give back exactly the text from before the merge.
    const e = editing("base");
    e.type(" mine");
    e.set("merged");
    e.undo();
    expect(e.text()).toBe("base mine");
    e.redo();
    expect(e.text()).toBe("merged");
  });
  it("is_not_joined_by_the_typing_that_follows_it", () => {
    const e = editing("base");
    e.set("merged");
    e.type("!");
    e.undo();
    expect(e.text()).toBe("merged");
    e.undo();
    expect(e.text()).toBe("base");
  });
  it("keeps_the_cursor_on_its_line_and_column", () => {
    const state = EditorState.create({ doc: "one\ntwo\nthree", selection: { anchor: 6 } });
    const next = state.update(outsideText(state, "zero\none\ntw", []));
    expect(next.state.selection.main.head).toBe("zero\no".length + 1);
  });
});

const props = { name: "a.ts", path: "C:/p/a.ts", initial: "let a = 1;\n", readOnly: false, wrap: false, onEdit: () => {}, onCursor: () => {} };

describe("Editor", () => {
  it("renders_a_host_with_the_footers_hint_as_its_description", () => {
    const html = renderToStaticMarkup(
      <>
        <Editor {...props} hintId="hint-1" />
        <EditorFooter line={1} column={1} encoding="utf-8" eol="lf" hintId="hint-1" />
      </>,
    );
    expect(html).toContain('class="editor__host"');
    expect(html).toContain('data-hint="hint-1"');
    expect(html).toContain('id="hint-1"');
    expect(html).toContain("Esc, then Tab, leaves the editor");
  });

  it("the_footer_shows_the_position_the_encoding_and_the_ending", () => {
    const footer = (eol: Eol, encoding: Encoding) => renderToStaticMarkup(<EditorFooter line={12} column={5} encoding={encoding} eol={eol} />);
    const html = footer("crlf", "utf-8-bom");
    expect(html).toContain("Ln 12, Col 5");
    expect(html).toContain("UTF-8 with BOM");
    expect(html).toContain("CRLF");
    expect(footer("lf", "utf-16le")).toContain("UTF-16 LE");
    expect(footer("lf", "utf-16be")).toContain("UTF-16 BE");
    expect(footer("lf", "utf-8")).toContain(">LF<");
  });
});
