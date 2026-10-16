import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./ReviewSheet.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};

describe("ReviewSheet.css", () => {
  it("marks the chosen file with a bar on its start edge as well as a fill", () => {
    expect(rule(".review__file[data-selected]")).toContain("border-inline-start: 3px solid var(--accent)");
    expect(rule(".review__file[data-selected]")).toContain("background: var(--selected)");
  });
  it("keeps the listbox's focus ring visible", () => {
    expect(bare).toContain(".review__files:focus-visible");
    expect(bare).not.toMatch(/outline:\s*none/);
  });
  it("caps the file list and scrolls inside it, so the diff beside it stays in view", () => {
    const list = rule(".review__files");
    expect(list).toMatch(/max-block-size:\s*calc\(\d+ \* var\(--space-6\)\)/);
    expect(list).toMatch(/overflow-y:\s*auto/);
    expect(list).toMatch(/overscroll-behavior:\s*contain/);
  });
  it("scrolls nowhere else: the page body scrolls the rest", () => {
    expect(bare.match(/overflow(?:-[xy])?:\s*(?:auto|scroll)/g)).toHaveLength(1);
  });
  it("dims an inert button, and gives it GrayText in Windows high contrast", () => {
    expect(rule('.review .settings__btn[aria-disabled="true"]')).toMatch(/opacity:\s*0\.6/);
    expect(css.slice(css.indexOf("@media (forced-colors: active)"))).toContain('[aria-disabled="true"]');
  });
  it("dims commands that are stale, and the cue above them has a bar on the start edge as well as the words", () => {
    expect(rule('.review__commands[data-stale="true"] .command-list')).toMatch(/opacity:\s*0\.6/);
    expect(rule(".review__stale")).toContain("border-inline-start: 3px solid var(--accent)");
  });
  it("marks a failure with a bar on the start edge as well as the words", () => {
    expect(rule(".review__error")).toContain("border-inline-start: 3px solid var(--error)");
  });
  it("only styles hover where the device hovers", () => {
    expect(bare.slice(0, bare.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("gives the list, the note and the banner an edge in Windows high contrast, and the chosen file a Highlight outline", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    for (const sel of [".review__files", ".review__note", ".review__banner"]) expect(forced, sel).toContain(sel);
    expect(forced).toMatch(/outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\[data-selected\][^}]*\{[^}]*outline:\s*2px solid Highlight/);
  });
  it("uses tokens and logical properties only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(bare).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
  });
});
