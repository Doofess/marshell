import { describe, expect, it } from "vitest";
import { focusOrder } from "./focusOrder";

describe("focusOrder", () => {
  it("walks what the dialog trap walks, an editor's contenteditable included", () => {
    const html = '<div><button>Save</button><div contenteditable="true" aria-label="Doc, editor"></div><div contenteditable="false" aria-label="Read only"></div><span tabindex="-1">x</span></div>';
    expect(focusOrder(html)).toEqual(["Save", "Doc, editor"]);
  });
});
