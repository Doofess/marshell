import { describe, expect, it } from "vitest";
import { FILES_DEFAULTS } from "./filesSettings";

describe("FILES_DEFAULTS", () => {
  it("are the core's defaults", () => {
    expect(FILES_DEFAULTS).toEqual({ preview_tabs: true, auto_save: false, tab_indents: true, word_wrap: false, indent_guides: true, whitespace: false, scroll_past_end: false, tab_size: 4, tree_position: "automatic" });
  });
});
