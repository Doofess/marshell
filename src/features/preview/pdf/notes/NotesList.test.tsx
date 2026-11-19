import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Note } from "./editor";
import { NotesList } from "./NotesList";
import { colourName, noteRows } from "./noteRows";

const note = (o: Partial<Note>): Note => ({ id: "a", page: 1, kind: "highlight", kindName: "Highlight", color: "#FFFF98", text: "quick brown", comment: "", editable: true, top: 0.1, ...o });
const noop = () => {};
const render = (notes: Note[], current: string | null = null) => renderToStaticMarkup(<NotesList name="spec.pdf" rows={noteRows(notes, colourName)} current={current} onPick={noop} />);
const rowOf = (html: string, id: string) => html.match(new RegExp(`<button[^>]*data-note="${id}"[^>]*>`))![0];

describe("NotesList markup", () => {
  it("is_a_list_named_for_the_file", () => {
    const html = render([note({})]);
    expect(html).toContain('role="list"');
    expect(html).toContain('aria-label="Notes in spec.pdf"');
    expect(html).toContain('aria-label="Highlight, Yellow, page 1: quick brown"');
  });

  it("shows_the_two_empty_state_lines", () => {
    const html = render([]);
    expect(html).toContain("This PDF has no notes yet.");
    expect(html).toContain("Select text to highlight it, or use the Free text tool.");
    expect(html).not.toContain('role="list"');
  });

  it("renders_nothing_while_the_notes_load", () => {
    expect(renderToStaticMarkup(<NotesList name="a.pdf" rows={null} current={null} onPick={noop} />)).toBe("");
  });

  it("marks_only_the_current_note_with_aria_current", () => {
    const html = render([note({ id: "a" }), note({ id: "b", top: 0.5 })], "b");
    expect(html.match(/aria-current="true"/g)).toHaveLength(1);
    expect(rowOf(html, "b")).toContain('aria-current="true"');
    expect(rowOf(html, "a")).not.toContain("aria-current");
  });

  it("gives_one_row_the_tab_stop_the_current_one_or_else_the_first", () => {
    const two = [note({ id: "a" }), note({ id: "b", top: 0.5 })];
    const withCurrent = render(two, "b");
    expect(withCurrent.match(/tabindex="0"/g)).toHaveLength(1);
    expect(rowOf(withCurrent, "b")).toContain('tabindex="0"');
    expect(rowOf(render(two), "a")).toContain('tabindex="0"');
  });

  it("draws_a_swatch_a_T_or_an_icon_by_kind_and_a_comment_under_the_text", () => {
    const html = render([
      note({ id: "h", comment: "Check this date" }),
      note({ id: "f", kind: "freetext", kindName: "Free text", color: null, text: "Sign here", top: 0.3 }),
      note({ id: "o", kind: "other", kindName: "Sticky note", color: null, text: "", editable: false, top: 0.6 }),
    ]);
    expect(html).toContain("background-color:#FFFF98");
    expect(html).toContain("pdf-notes__tee");
    expect(html).toContain("pdf-notes__icon");
    expect(html).toContain("Check this date");
  });
});
