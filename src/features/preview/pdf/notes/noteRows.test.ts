import { describe, expect, it } from "vitest";
import type { Note } from "./editor";
import { colourName, currentNoteId, noteRows } from "./noteRows";

const note = (o: Partial<Note>): Note => ({ id: "a", page: 1, kind: "highlight", kindName: "Highlight", color: "#FFFF98", text: "quick brown", comment: "", editable: true, top: 0.1, ...o });
const name = colourName;

describe("noteRows", () => {
  it("noteRows_sorts_by_page_then_top", () => {
    const rows = noteRows([note({ id: "c", page: 2, top: 0.1 }), note({ id: "b", page: 1, top: 0.5 }), note({ id: "a", page: 1, top: 0.2 })], name);
    expect(rows.map((r) => r.id)).toEqual(["a", "b", "c"]);
  });

  it("noteRows_reads_a_highlight_with_its_colour_name", () => {
    const [r] = noteRows([note({ page: 3 })], name);
    expect(r!.kindText).toBe("Highlight, Yellow");
    expect(r!.label).toBe("Highlight, Yellow, page 3: quick brown");
    expect(r!.text).toBe("“quick brown”");
    expect(r!.page).toBe(3);
  });

  it("noteRows_names_the_five_colours_in_any_case_and_no_other", () => {
    expect(colourName("#53ffbc")).toBe("Green");
    expect(colourName("#80EBFF")).toBe("Blue");
    expect(colourName("#FFCBE6")).toBe("Pink");
    expect(colourName("#FF4F5F")).toBe("Red");
    expect(colourName("#FFFF00")).toBeNull();
    expect(colourName(null)).toBeNull();
  });

  it("noteRows_leaves_an_unnamed_colour_out_of_the_kind", () => {
    const [r] = noteRows([note({ color: "#FFFF00" })], name);
    expect(r!.kindText).toBe("Highlight");
    expect(r!.label).toBe("Highlight, page 1: quick brown");
    expect(r!.swatch).toBe("#FFFF00");
  });

  it("noteRows_puts_a_comment_on_its_own_line_and_none_when_empty", () => {
    const [with_, without] = noteRows([note({ id: "a", comment: "Check this date" }), note({ id: "b", top: 0.9 })], name);
    expect(with_!.comment).toBe("Check this date");
    expect(with_!.label).toBe("Highlight, Yellow, page 1: quick brown. Check this date");
    expect(without!.comment).toBe("");
    expect(without!.label.endsWith("brown")).toBe(true);
  });

  it("noteRows_reads_free_text_with_no_colour", () => {
    const [r] = noteRows([note({ kind: "freetext", kindName: "Free text", color: null, text: "Sign here", page: 7 })], name);
    expect(r!.kindText).toBe("Free text");
    expect(r!.label).toBe("Free text, page 7: Sign here");
    expect(r!.text).toBe("Sign here");
    expect(r!.swatch).toBeNull();
  });

  it("noteRows_marks_another_kind_read_only", () => {
    const [r] = noteRows([note({ kind: "other", kindName: "Sticky note", color: null, text: "", comment: "Hi", editable: false })], name);
    expect(r!.kindText).toBe("Sticky note, read-only");
    expect(r!.text).toBe("Sticky note, read-only");
    expect(r!.label).toBe("Sticky note, read-only, page 1. Hi");
    expect(r!.editable).toBe(false);
  });

  it("noteRows_adds_the_WinAnsi_line_to_free_text_the_editor_made_only", () => {
    const free = { kind: "freetext", kindName: "Free text", color: null, text: "Привет" } as const;
    const [made, file] = noteRows([note({ ...free, id: "m", added: true }), note({ ...free, id: "f", top: 0.9 })], name);
    expect(made!.warning).toBe("May show blank in Edge and Chrome.");
    expect(made!.label).toBe("Free text, page 1: Привет. May show blank in Edge and Chrome.");
    expect(file!.warning).toBeNull();
    const [plain] = noteRows([note({ kind: "freetext", kindName: "Free text", color: null, text: "Sign here", added: true })], name);
    expect(plain!.warning).toBeNull();
  });

  it("currentNoteId_is_the_first_note_on_the_page_in_view", () => {
    const rows = noteRows([note({ id: "a", page: 1 }), note({ id: "b", page: 3, top: 0.1 }), note({ id: "c", page: 3, top: 0.6 })], name);
    expect(currentNoteId(rows, 3)).toBe("b");
    expect(currentNoteId(rows, 2)).toBeNull();
  });
});
