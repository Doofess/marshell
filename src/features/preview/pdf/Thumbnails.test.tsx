import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Thumbnails } from "./Thumbnails";

const sizes = [
  { w: 600, h: 800 },
  { w: 600, h: 800 },
  { w: 800, h: 600 },
];
const render = (current = 2) => renderToStaticMarkup(<Thumbnails doc={{} as never} sizes={sizes} current={current} name="spec.pdf" onPick={() => {}} focusOnShow={false} />);

describe("Thumbnails markup", () => {
  it("is_a_list_named_for_the_file", () => {
    const html = render();
    expect(html).toContain('role="list"');
    expect(html).toContain('aria-label="Pages of spec.pdf"');
    expect(html).toContain('class="pdf-thumbs"');
  });
  it("has_a_button_named_page_n_with_the_number_shown_under_the_plate", () => {
    const html = render();
    expect(html).toContain('aria-label="Page 3"');
    expect(html).toMatch(/<span class="pdf-thumbs__num">3<\/span>/);
    expect((html.match(/<button/g) ?? []).length).toBe(3);
    expect(html).toContain('data-thumb="3"');
    expect(html).not.toContain("data-page=");
  });
  it("marks_only_the_page_in_view_as_current_and_gives_it_the_tab_stop", () => {
    const html = render(2);
    expect((html.match(/aria-current="true"/g) ?? []).length).toBe(1);
    expect(html).toMatch(/data-thumb="2"[^>]*aria-current="true"|aria-current="true"[^>]*data-thumb="2"/);
    expect((html.match(/tabindex="0"/g) ?? []).length).toBe(1);
    expect((html.match(/tabindex="-1"/g) ?? []).length).toBe(2);
  });
  it("shapes_each_plate_by_its_pages_ratio_at_160_px_wide", () => {
    const html = render();
    expect(html).toContain("inline-size:160px;block-size:213.33");
    expect(html).toContain("inline-size:160px;block-size:120px");
  });
});
