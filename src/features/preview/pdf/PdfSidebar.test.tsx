import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PdfSidebar } from "./PdfSidebar";
import type { OutlineNode } from "./outline";

const noop = () => {};
const node = (id: string, title: string, page: number | null, children: OutlineNode[] = [], goal: OutlineNode["goal"] = { kind: "dest", dest: "x" }): OutlineNode => ({ id, title, goal, page, open: false, children });
const outline = [node("0", "Intro", 1, [node("0.0", "Scope", 3)]), node("1", "Design", 12), node("2", "Appendix", null, [], null)];

const render = (p: Partial<Parameters<typeof PdfSidebar>[0]> = {}) =>
  renderToStaticMarkup(<PdfSidebar id="side" name="spec.pdf" view="outline" onView={noop} outline={outline} section="1" expanded={new Set(["0"])} onToggle={noop} onPick={noop} onLeave={noop} pages={null} {...p} />);

describe("PdfSidebar markup", () => {
  it("is_a_landmark_named_for_its_job_and_never_a_dialog_or_menu", () => {
    const html = render();
    expect(html).toContain('<nav id="side"');
    expect(html).toContain('aria-label="Outline and pages"');
    expect(html).not.toMatch(/role="(dialog|menu|alertdialog)"|aria-modal/);
  });

  it("has_a_tab_list_with_the_outline_and_pages_tabs_and_the_chosen_one_selected", () => {
    const html = render();
    expect(html).toContain('role="tablist"');
    expect(html).toContain('aria-label="Sidebar views"');
    expect(html).toMatch(/role="tab"[^>]*aria-selected="true"[^>]*>Outline</);
    expect(html).toMatch(/role="tab"[^>]*aria-selected="false"[^>]*>Pages</);
    expect(render({ view: "pages" })).toMatch(/role="tab"[^>]*aria-selected="true"[^>]*>Pages</);
  });

  it("has_a_tree_with_levels_sets_and_expanded_states", () => {
    const html = render();
    expect(html).toContain('role="tree"');
    expect(html).toContain('aria-label="Outline of spec.pdf"');
    // Open parent, its child at level 2, and a closed-by-default sibling; the leaf has no aria-expanded.
    expect(html).toMatch(/data-id="0"[^>]*aria-level="1"[^>]*aria-setsize="3"[^>]*aria-posinset="1"[^>]*aria-expanded="true"/);
    expect(html).toMatch(/data-id="0.0"[^>]*aria-level="2"[^>]*aria-setsize="1"[^>]*aria-posinset="1"/);
    expect(html.match(/data-id="1"[^>]*>/)![0]).not.toContain("aria-expanded");
    expect(render({ expanded: new Set() })).not.toContain('data-id="0.0"');
  });

  it("gives_one_row_the_tab_stop_the_section_when_it_shows", () => {
    const html = render();
    expect(html.match(/tabindex="0"/g)!.length).toBe(2); // the selected tab and one row
    expect(html).toMatch(/data-id="1"[^>]*tabindex="0"/);
    expect(render({ section: null })).toMatch(/data-id="0"[^>]*tabindex="0"/);
  });

  it("marks_the_section_with_aria_current_and_only_that_row", () => {
    const html = render();
    expect(html.match(/aria-current="true"/g)).toHaveLength(1);
    expect(html).toMatch(/data-id="1"[^>]*aria-current="true"/);
    expect(html).toContain("is-selected-mark");
  });

  it("hides_the_arrow_and_the_page_number_from_the_name_and_shows_the_title", () => {
    const html = render();
    expect(html).toContain('<span class="pdf-sidebar__page" aria-hidden="true">12</span>');
    expect(html).toContain('<span class="pdf-sidebar__title">Design</span>');
    expect(html).toMatch(/<span class="pdf-sidebar__arrow" aria-hidden="true">/);
  });

  it("shows_no_page_number_for_an_entry_without_a_page", () => {
    const html = render();
    const row = html.match(/data-id="2".*?<\/div>/)![0];
    expect(row).not.toContain("pdf-sidebar__page");
  });

  it("says_so_with_a_show_pages_button_when_the_outline_is_empty", () => {
    const html = render({ outline: [], section: null });
    expect(html).toContain("This PDF has no outline.");
    expect(html).toMatch(/<button[^>]*>Show pages<\/button>/);
    expect(html).not.toContain('role="tree"');
  });

  it("shows_nothing_in_the_outline_view_while_the_outline_loads", () => {
    const html = render({ outline: null, section: null });
    expect(html).not.toContain('role="tree"');
    expect(html).not.toContain("no outline");
  });

  it("shows_the_pages_slot_in_the_pages_view_and_not_the_tree", () => {
    const html = render({ view: "pages", pages: <ul className="slot" /> });
    expect(html).toContain('class="slot"');
    expect(html).not.toContain('role="tree"');
  });
});

describe("the sidebar's styles", () => {
  const css = readFileSync(new URL("./PdfView.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const body = (sel: string) => css.match(new RegExp(`(?:^|\\})\\s*${sel.replace(/[.[\]"=]/g, "\\$&")}\\s*\\{([^}]*)\\}`, "m"))?.[1] ?? "";
  it("lets_only_the_tree_scroll_inside_the_sidebar", () => {
    expect(body(".pdf-sidebar__tree")).toMatch(/overflow-y:\s*auto/);
    expect(body(".pdf-sidebar")).not.toMatch(/overflow(?:-y)?:\s*(?:auto|scroll)/);
  });
  it("sticks_the_sidebar_under_the_head", () => {
    expect(body(".pdf-sidebar")).toMatch(/position:\s*sticky/);
  });
  it("keeps_every_hover_inside_the_hover_media_query", () => {
    const outside = css.replace(/@media \(hover: hover\) and \(pointer: fine\) \{[^@]*?\n\}/g, "").replace(/@media \(forced-colors: active\) and \(hover: hover\) and \(pointer: fine\) \{[^@]*?\n\}/g, "");
    expect(outside).not.toMatch(/\.pdf-sidebar[^{}]*:hover/);
  });
  it("uses_no_transitions", () => {
    expect(css).not.toMatch(/transition/);
  });
});
