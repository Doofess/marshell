import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PdfToolbar } from "./PdfToolbar";

const noop = () => {};
const html = renderToStaticMarkup(
  <PdfToolbar
    page="12"
    count={140}
    percent={110}
    fit={true}
    findOpen={false}
    sidebar={<button type="button" className="pdf__button" aria-label="Outline and pages" />}
    onPageDraft={noop}
    onPageCommit={noop}
    onPageCancel={noop}
    onZoomOut={noop}
    onZoomIn={noop}
    onFit={noop}
    onFind={noop}
    findRow={<div className="pdf__find" />}
  />,
);

describe("PdfToolbar markup", () => {
  it("has_its_controls_in_the_copy_order", () => {
    const at = (s: string) => html.indexOf(s);
    const order = ["Outline and pages", 'aria-label="Page, of 140"', 'aria-label="Zoom out"', "110%", 'aria-label="Zoom in"', 'aria-label="Fit width"', 'aria-label="Find"', 'class="pdf__find"'].map(at);
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
  });
  it("has_titles_and_key_shortcuts", () => {
    expect(html).toContain('title="Zoom out (Ctrl+-)"');
    expect(html).toContain('title="Zoom in (Ctrl+=)"');
    expect(html).toContain('title="Fit width (Ctrl+0)"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain('title="Find (Ctrl+F)"');
    expect(html).toContain('aria-keyshortcuts="Control+F"');
    expect(html).toContain('aria-expanded="false"');
  });
  it("shows_no_text_on_the_icon_buttons", () => {
    const buttons = [...html.matchAll(/<button[^>]*aria-label="(Zoom out|Zoom in|Fit width|Find)"[^>]*>([\s\S]*?)<\/button>/g)];
    expect(buttons).toHaveLength(4);
    for (const b of buttons) expect(b[2]!.replace(/<svg[\s\S]*?<\/svg>/g, "")).toBe("");
  });
  it("keeps_the_word_page_in_its_own_element", () => {
    expect(html).toContain('<span class="pdf__word">Page</span>');
    expect(html).toContain("of 140");
  });
});
