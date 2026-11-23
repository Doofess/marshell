import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./PullSheets.css", import.meta.url), "utf8");
const panel = readFileSync(new URL("./ChangesPanel.tsx", import.meta.url), "utf8");
/** The rule whose selector list starts with `selector`, up to its closing brace. */
const rule = (selector: string) => {
  const at = css.indexOf(`\n${selector}`);
  return at < 0 ? "" : css.slice(at, css.indexOf("}", at));
};

describe("the pull request sheets' styles", () => {
  it("styles an off button only inside these sheets, by fill and ink and never by opacity", () => {
    expect(css.split("\n").some((line) => line.startsWith(".close-dialog__btn[aria-disabled"))).toBe(false);
    const off = rule('.pull-sheet .close-dialog__btn[aria-disabled="true"]');
    expect(off).toContain("background: var(--hover)");
    expect(off).toContain("color: var(--text-3)");
    expect(off).not.toContain("opacity");
  });
  it("keeps an off button's fill under the pointer", () => {
    expect(css).toContain('.pull-sheet .close-dialog__btn[aria-disabled="true"]:hover');
  });
  it("lets the description grow with its text, so the sheet's body stays its one scroller", () => {
    const text = rule(".pull-sheet__text");
    expect(text).toContain("field-sizing: content");
    expect(text).toContain("resize: none");
  });
});

describe("where the sheets are drawn", () => {
  it("puts them over the whole window, like every other dialog, not inside the side pane", () => {
    expect(panel).toContain("createPortal(");
    expect(panel).toContain('closest(".live-app")');
  });
});
