import { describe, expect, it } from "vitest";
import { FILES_DEFAULTS } from "../files/filesSettings";
import { EDITOR_DEFAULTS } from "./editorPrefs";

describe("EDITOR_DEFAULTS", () => {
  it("are the Files defaults for the six editor settings, and no others", () => {
    const { tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size } = FILES_DEFAULTS;
    expect(EDITOR_DEFAULTS).toEqual({ tab_indents, word_wrap, indent_guides, whitespace, scroll_past_end, tab_size });
  });
});
