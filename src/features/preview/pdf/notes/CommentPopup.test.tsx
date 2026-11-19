import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { CommentRequest } from "./editor";
import { CommentPopup, popupKey, popupPlace } from "./CommentPopup";

const key = (k: string, mods: Partial<{ ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean; isComposing: boolean }> = {}) => ({
  key: k,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("popupKey", () => {
  it("popupKey_ctrl_enter_is_done", () => {
    expect(popupKey(key("Enter", { ctrlKey: true }))).toBe("done");
  });
  it("popupKey_plain_escape_is_cancel", () => {
    expect(popupKey(key("Escape"))).toBe("cancel");
  });
  it("popupKey_enter_types_a_newline", () => {
    expect(popupKey(key("Enter"))).toBeNull();
    expect(popupKey(key("Enter", { shiftKey: true }))).toBeNull();
  });
  it("popupKey_other_keys_and_modified_escape_are_not_taken", () => {
    expect(popupKey(key("a"))).toBeNull();
    expect(popupKey(key("Tab"))).toBeNull();
    expect(popupKey(key("Escape", { ctrlKey: true }))).toBeNull();
    expect(popupKey(key("Escape", { shiftKey: true }))).toBeNull();
    expect(popupKey(key("Enter", { ctrlKey: true, shiftKey: true }))).toBeNull();
    expect(popupKey(key("Enter", { ctrlKey: true, altKey: true }))).toBeNull();
  });
  it("popupKey_nothing_while_composing", () => {
    expect(popupKey(key("Escape", { isComposing: true }))).toBeNull();
    expect(popupKey(key("Enter", { ctrlKey: true, isComposing: true }))).toBeNull();
  });
});

describe("popupPlace", () => {
  const size = { width: 280, height: 120 };
  const page = { left: 50, top: 20, right: 650, bottom: 820 };
  const at = (left: number, top: number, w = 100, h = 20) => ({ left, top, right: left + w, bottom: top + h });

  it("popupPlace_goes_right_of_the_note_with_its_top_at_the_notes", () => {
    // The note's right edge is 250, so the popup starts 8 px on, at 258; both are relative to the page's box.
    expect(popupPlace(at(150, 100), page, size)).toEqual({ left: 258 - 50, top: 100 - 20 });
  });
  it("popupPlace_flips_left_when_the_right_would_pass_the_page", () => {
    const note = at(450, 100);
    // Right: 558 + 280 = 838 > 650. Left: 450 - 8 - 280 = 162.
    expect(popupPlace(note, page, size)).toEqual({ left: 162 - 50, top: 100 - 20 });
  });
  it("popupPlace_goes_under_the_note_when_neither_side_fits", () => {
    const narrow = { left: 0, top: 0, right: 300, bottom: 800 };
    // The note is 100 wide at x 100; the right and the left both pass the page. Under it, clamped inside the page: 300 - 280 = 20.
    expect(popupPlace(at(100, 200), narrow, size)).toEqual({ left: 20, top: 228 });
  });
  it("popupPlace_under_the_note_keeps_the_notes_left_when_it_fits", () => {
    const narrow = { left: 0, top: 0, right: 400, bottom: 800 };
    expect(popupPlace(at(60, 200, 340), narrow, size)).toEqual({ left: 60, top: 228 });
  });
  it("popupPlace_is_clamped_at_the_bottom_of_the_page", () => {
    expect(popupPlace(at(150, 780), page, size)).toEqual({ left: 258 - 50, top: 820 - 120 - 20 });
  });
  it("popupPlace_is_clamped_at_the_top_of_the_page", () => {
    expect(popupPlace(at(150, -40), page, size)).toEqual({ left: 258 - 50, top: 0 });
  });
});

describe("CommentPopup markup", () => {
  const request = (text: string): CommentRequest => ({ page: 7, anchor: {} as DOMRect, note: {} as HTMLElement, text, done() {} });
  const html = (text: string) => renderToStaticMarkup(<CommentPopup request={request(text)} onClose={() => {}} />);

  it("is_a_dialog_named_comment_that_is_not_modal", () => {
    const h = html("");
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-label="Comment"');
    expect(h).not.toContain("aria-modal");
  });
  it("has_a_textarea_named_with_the_page_that_holds_the_comment", () => {
    const h = html("Check this date");
    expect(h).toContain('aria-label="Comment on the note on page 7"');
    expect(h).toMatch(/<textarea[^>]*>Check this date<\/textarea>/);
  });
  it("has_cancel_and_done_with_their_keys_in_the_titles_done_last", () => {
    const h = html("");
    expect(h).toContain('title="Cancel (Esc)"');
    expect(h).toContain('title="Done (Ctrl+Enter)"');
    expect(h.indexOf(">Cancel</button>")).toBeGreaterThan(0);
    expect(h.indexOf(">Done</button>")).toBeGreaterThan(h.indexOf(">Cancel</button>"));
  });
  it("has_delete_comment_first_only_when_the_note_had_a_comment", () => {
    const had = html("Check this date");
    expect(had).toContain(">Delete comment</button>");
    expect(had.indexOf(">Delete comment</button>")).toBeLessThan(had.indexOf(">Cancel</button>"));
    expect(html("")).not.toContain("Delete comment");
    expect(html("  \n ")).not.toContain("Delete comment");
  });
});
