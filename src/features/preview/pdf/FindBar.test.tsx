import { renderToStaticMarkup } from "react-dom/server";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { elements } from "../../../test/hooks";
import { FindBar, countText, fieldKey, pdfKey, selectionText, stepText } from "./FindBar";
import { IDLE, type FindStatus } from "./findText";

// A letter's code is its key on a US layout unless the test says where it sits.
const key = (k: string, o: Partial<{ code: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean }> = {}) =>
  pdfKey({ key: k, code: k.length === 1 ? `Key${k.toUpperCase()}` : k, ctrlKey: false, altKey: false, metaKey: false, shiftKey: false, ...o });
const found = (over: Partial<FindStatus> = {}): FindStatus => ({ state: "found", current: 3, total: 17, page: 5, wrapped: null, ...over });
const NO_TEXT = "This PDF has no text to search. Its pages may be scanned images.";

describe("pdfKey", () => {
  it("pdfKey_ctrl_f_is_find", () => expect(key("f", { ctrlKey: true })).toBe("find"));
  it("pdfKey_f3_is_next", () => expect(key("F3")).toBe("next"));
  it("pdfKey_shift_f3_is_previous", () => expect(key("F3", { shiftKey: true })).toBe("previous"));
  it("pdfKey_f4_is_sidebar", () => expect(key("F4")).toBe("sidebar"));
  it("pdfKey_other_modifiers_give_null", () => {
    expect(key("f", { ctrlKey: true, shiftKey: true })).toBeNull();
    expect(key("F3", { altKey: true })).toBeNull();
    expect(key("F3", { ctrlKey: true })).toBeNull();
    expect(key("F4", { shiftKey: true })).toBeNull();
    expect(key("f")).toBeNull();
  });
  // As the app's own shortcuts (lib/keymap.ts): the same key on a Hebrew, Cyrillic or Dvorak keyboard.
  it("pdfKey_ctrl_f_is_the_physical_key", () => {
    expect(key("כ", { ctrlKey: true, code: "KeyF" })).toBe("find");
    expect(key("f", { ctrlKey: true, code: "KeyY" })).toBeNull();
  });
});

describe("fieldKey", () => {
  const k = (key: string, o: Partial<{ shiftKey: boolean; isComposing: boolean }> = {}) => fieldKey({ key, shiftKey: false, isComposing: false, ...o });
  it("fieldKey_enter_steps_and_esc_closes", () => {
    expect(k("Enter")).toBe("next");
    expect(k("Enter", { shiftKey: true })).toBe("previous");
    expect(k("Escape")).toBe("close");
    expect(k("a")).toBeNull();
  });
  it("fieldKey_leaves_an_ime_composition_its_keys", () => {
    // Enter confirms the composition and Esc cancels it; neither steps nor closes the bar.
    expect(k("Enter", { isComposing: true })).toBeNull();
    expect(k("Escape", { isComposing: true })).toBeNull();
  });
});

describe("countText", () => {
  it("countText_is_empty_for_a_blank_query_or_idle", () => {
    expect(countText(found(), "  ")).toBe("");
    expect(countText(IDLE, "a")).toBe("");
  });
  it("countText_found_and_pending", () => {
    expect(countText(found(), "a")).toBe("3 of 17");
    expect(countText({ ...found(), state: "pending" }, "a")).toBe("3 of 17");
    expect(countText({ ...IDLE, state: "pending" }, "a")).toBe("");
  });
  it("countText_not_found_and_no_text", () => {
    expect(countText({ ...IDLE, state: "notFound" }, "a")).toBe("No matches");
    expect(countText({ ...IDLE, state: "noText" }, "a")).toBe(NO_TEXT);
  });
});

describe("stepText", () => {
  it("stepText_found_with_and_without_a_wrap", () => {
    expect(stepText(found())).toBe("Match 3 of 17, page 5.");
    expect(stepText(found({ wrapped: "start" }))).toBe("Match 3 of 17, page 5. Continued from the start.");
    expect(stepText(found({ wrapped: "end" }))).toBe("Match 3 of 17, page 5. Continued from the end.");
  });
  it("stepText_other_states", () => {
    expect(stepText({ ...IDLE, state: "notFound" })).toBe("No matches.");
    expect(stepText({ ...IDLE, state: "noText" })).toBe(NO_TEXT);
    expect(stepText(IDLE)).toBe("");
    expect(stepText({ ...IDLE, state: "pending" })).toBe("");
  });
});

describe("selectionText", () => {
  it("selectionText_takes_a_selection_inside_the_pdf", () => expect(selectionText({ text: "  hello ", inside: true })).toBe("hello"));
  it("selectionText_refuses_outside_the_pdf", () => expect(selectionText({ text: "hello", inside: false })).toBeNull());
  it("selectionText_refuses_two_lines", () => expect(selectionText({ text: "a\nb", inside: true })).toBeNull());
  it("selectionText_refuses_201_characters_and_keeps_200", () => {
    expect(selectionText({ text: "x".repeat(201), inside: true })).toBeNull();
    expect(selectionText({ text: "x".repeat(200), inside: true })).toBe("x".repeat(200));
  });
  it("selectionText_refuses_spaces_only", () => expect(selectionText({ text: "   ", inside: true })).toBeNull());
});

describe("FindBar markup", () => {
  const html = renderToStaticMarkup(
    <FindBar query={{ text: "cat", caseSensitive: false, entireWord: false }} status={found()} onQuery={() => {}} onStep={() => {}} onClose={() => {}} field={null} />,
  );
  it("has_the_field_label_buttons_and_checkboxes", () => {
    expect(html).toContain('aria-label="Find in PDF"');
    expect(html).toContain('placeholder="Find"');
    expect(html).toContain('aria-label="Previous"');
    expect(html).toContain('title="Previous match (Shift+Enter)"');
    expect(html).toContain('aria-label="Next"');
    expect(html).toContain('title="Next match (Enter)"');
    expect(html).toContain("Match case");
    expect(html).toContain("Whole word");
    expect(html.match(/type="checkbox"/g)).toHaveLength(2);
    expect(html).toContain('aria-label="Close find bar"');
  });
  it("shows_the_count_in_an_element_with_no_live_region", () => {
    expect(html).toMatch(/<span class="pdf__count"[^>]*>3 of 17<\/span>/);
    expect(html).not.toContain("aria-live");
    expect(html).not.toContain("role=\"status\"");
  });
});

// On "Match case" or "Next", Esc went to the pane, which took the keyboard to the terminal and left the bar and its tints up.
describe("FindBar keys", () => {
  const esc = () => ({ key: "Escape", shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, nativeEvent: { isComposing: false }, preventDefault: vi.fn() });
  const bar = (onClose: () => void) =>
    FindBar({ query: { text: "cat", caseSensitive: false, entireWord: false }, status: found(), onQuery: () => {}, onStep: () => {}, onClose, field: null }) as ReactElement<{ onKeyDown?: (e: unknown) => void }>;

  it("findBar_esc_on_a_button_closes_the_bar", () => {
    const onClose = vi.fn();
    const e = esc();
    bar(onClose).props.onKeyDown?.(e);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("findBar_esc_in_the_field_closes_the_bar_once_as_it_bubbles", () => {
    const onClose = vi.fn();
    const root = bar(onClose);
    const field = elements(root).find((el) => el.type === "input")!;
    const e = esc();
    field.props.onKeyDown(e);
    root.props.onKeyDown?.(e);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
