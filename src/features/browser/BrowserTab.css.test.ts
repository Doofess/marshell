import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { PAGE_GUTTER } from "./usePageSlot";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./BrowserTab.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("BrowserTab.css", () => {
  // The page sits PAGE_GUTTER inside the slot, and its frozen picture is the page's size: drawn over the whole slot, it would stretch.
  it("pads the slot by the page's gutter, so the picture and the fill land where the page was", () => {
    expect(rule(".browser-tab__slot")).toContain(`padding: ${PAGE_GUTTER}px;`);
  });
});
