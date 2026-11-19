import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { NoteTool } from "./notes/editor";
import { PdfToolbar } from "./PdfToolbar";

const noop = () => {};
const render = (tool: NoteTool, blocked: string | null) =>
  renderToStaticMarkup(
  <PdfToolbar
    tool={tool}
    blocked={blocked}
    onTool={noop}
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
const html = render("none", null);

describe("PdfToolbar markup", () => {
  it("has_its_controls_in_the_copy_order", () => {
    const at = (s: string) => html.indexOf(s);
    const order = ["Outline and pages", 'aria-label="Page, of 140"', 'aria-label="Zoom out"', "110%", 'aria-label="Zoom in"', 'aria-label="Fit width"', 'aria-label="Highlight"', 'aria-label="Free text"', 'aria-label="Find"', 'class="pdf__find"'].map(at);
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

describe("PdfToolbar note tools", () => {
  const tool = (h: string, label: string) => h.match(new RegExp(`<button[^>]*aria-label="${label}"[^>]*>`))?.[0] ?? "";
  it("has_the_two_tools_with_titles_and_no_pressed_one", () => {
    expect(tool(html, "Highlight")).toContain('title="Highlight: drag across text"');
    expect(tool(html, "Highlight")).toContain('aria-pressed="false"');
    expect(tool(html, "Free text")).toContain('title="Free text: click where it goes, or press Enter"');
    expect(tool(html, "Free text")).toContain('aria-pressed="false"');
  });
  it("shows_the_chosen_tool_pressed", () => {
    expect(tool(render("highlight", null), "Highlight")).toContain('aria-pressed="true"');
    expect(tool(render("highlight", null), "Free text")).toContain('aria-pressed="false"');
    expect(tool(render("freetext", null), "Free text")).toContain('aria-pressed="true"');
  });
  it("is_aria_disabled_with_the_reason_as_the_title_when_blocked", () => {
    const h = render("none", "a.pdf is read-only.");
    for (const l of ["Highlight", "Free text"]) {
      // aria-disabled, not disabled: the tool keeps its place in the tab order and says why it does nothing (PLAN.md's top bar).
      expect(tool(h, l)).toContain('aria-disabled="true"');
      expect(tool(h, l)).not.toMatch(/ disabled=""/);
      expect(tool(h, l)).toContain('title="Notes can&#x27;t be added: a.pdf is read-only."');
    }
  });
  it("leaves_the_tools_enabled_when_notes_can_be_added", () => {
    for (const l of ["Highlight", "Free text"]) expect(tool(html, l)).not.toMatch(/disabled/);
  });
  it("keeps_the_percentage_in_the_zoom_buttons_description", () => {
    const id = html.match(/<span class="pdf__percent" id="([^"]+)"/)?.[1];
    expect(id).toBeTruthy();
    expect(html.match(new RegExp(`aria-describedby="${id}"`, "g"))).toHaveLength(2);
  });
});
