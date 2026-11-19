import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { goalOf, handKeyboardOn, linksOf, PageLinks, type PageLink } from "./links";

describe("goalOf", () => {
  it("goalOf_http_and_https_are_web", () => {
    expect(goalOf({ url: "http://example.com/a" })).toEqual({ kind: "web", url: "http://example.com/a" });
    expect(goalOf({ url: "https://example.com/" })).toEqual({ kind: "web", url: "https://example.com/" });
  });
  it("goalOf_other_schemes_and_paths_are_other", () => {
    for (const url of ["javascript:alert(1)", "file:///c:/x", "mailto:a@b.c", "../other.pdf", "not a url"]) {
      expect(goalOf({ url })).toEqual({ kind: "other" });
    }
  });
  it("goalOf_reads_url_not_unsafeUrl", () => {
    expect(goalOf({ unsafeUrl: "https://example.com/" } as never)).toEqual({ kind: "other" });
  });
  it("goalOf_a_dest", () => {
    expect(goalOf({ dest: "chap" })).toEqual({ kind: "dest", dest: "chap" });
    expect(goalOf({ dest: [0, { name: "Fit" }] })).toEqual({ kind: "dest", dest: [0, { name: "Fit" }] });
  });
  it("goalOf_each_named_action", () => {
    for (const action of ["NextPage", "PrevPage", "FirstPage", "LastPage", "GoBack", "GoForward"]) {
      expect(goalOf({ action })).toEqual({ kind: "action", action });
    }
  });
  it("goalOf_an_unknown_action_is_other", () => {
    expect(goalOf({ action: "Print" })).toEqual({ kind: "other" });
    expect(goalOf({})).toEqual({ kind: "other" });
  });
  it("goalOf_checks_url_then_dest_then_action", () => {
    expect(goalOf({ url: "https://a.b/", dest: "x", action: "NextPage" }).kind).toBe("web");
    expect(goalOf({ dest: "x", action: "NextPage" }).kind).toBe("dest");
  });
});

// A page 600 wide and 800 high; the PDF's y runs up.
const view = {
  width: 600,
  height: 800,
  toView: (r: readonly number[]) => [r[0]!, 800 - r[1]!, r[2]!, 800 - r[3]!],
};

describe("linksOf", () => {
  it("linksOf_keeps_links_only_as_fractions", () => {
    const out = linksOf(
      [
        { subtype: "Link", rect: [60, 700, 180, 720], url: "https://example.com/" },
        { subtype: "Text", rect: [0, 0, 10, 10] },
      ],
      view,
    );
    expect(out).toHaveLength(1);
    expect(out[0]!.goal).toEqual({ kind: "web", url: "https://example.com/" });
    expect(out[0]!.label).toBe("https://example.com/");
    expect(out[0]!.box.left).toBeCloseTo(0.1);
    expect(out[0]!.box.width).toBeCloseTo(0.2);
    expect(out[0]!.box.top).toBeCloseTo(0.1);
    expect(out[0]!.box.height).toBeCloseTo(0.025);
  });
  it("linksOf_normalises_a_flipped_rectangle", () => {
    const [a] = linksOf([{ subtype: "Link", rect: [180, 720, 60, 700], dest: "x" }], view);
    expect(a!.box.left).toBeCloseTo(0.1);
    expect(a!.box.top).toBeCloseTo(0.1);
    expect(a!.box.width).toBeCloseTo(0.2);
    expect(a!.box.height).toBeCloseTo(0.025);
  });
  it("linksOf_labels", () => {
    const labels = (a: object) => linksOf([{ subtype: "Link", rect: [0, 0, 10, 10], ...a }], view)[0]!.label;
    expect(labels({ dest: "x" })).toBe("Link within the file");
    expect(labels({ action: "NextPage" })).toBe("Next page");
    expect(labels({ action: "PrevPage" })).toBe("Previous page");
    expect(labels({ action: "FirstPage" })).toBe("First page");
    expect(labels({ action: "LastPage" })).toBe("Last page");
    expect(labels({ action: "GoBack" })).toBe("Back");
    expect(labels({ action: "GoForward" })).toBe("Forward");
    expect(labels({ url: "mailto:a@b.c" })).toBe("Link");
  });
});

describe("PageLinks", () => {
  const links: PageLink[] = [
    { box: { left: 0.1, top: 0.2, width: 0.3, height: 0.05 }, goal: { kind: "web", url: "https://example.com/" }, label: "https://example.com/" },
    { box: { left: 0, top: 0, width: 0.5, height: 0.5 }, goal: { kind: "dest", dest: "x" }, label: "Link within the file" },
  ];
  const html = renderToStaticMarkup(<PageLinks links={links} onFollow={() => {}} />);
  it("PageLinks_are_spans_with_role_link_and_no_href", () => {
    expect(html.match(/role="link"/g)).toHaveLength(2);
    expect(html).toContain('tabindex="0"');
    expect(html).not.toContain("href");
    expect(html).not.toContain("<a ");
  });
  it("PageLinks_labels_and_a_title_on_a_web_link_only", () => {
    expect(html).toContain('aria-label="https://example.com/"');
    expect(html).toContain('aria-label="Link within the file"');
    expect(html.match(/title="/g)).toHaveLength(1);
    expect(html).toContain('title="https://example.com/"');
  });
  it("PageLinks_are_placed_in_percentages", () => {
    expect(html).toContain("10%");
    expect(html).toContain("30%");
  });
  // A page's geometry is physical, as pdf.js's text layer is: a right-to-left pane must not mirror the links off their words.
  it("PageLinks_are_placed_by_physical_left_and_top", () => {
    expect(html).toMatch(/left:10%;top:20%;width:30%;height:5%/);
    expect(html).not.toContain("inset-inline-start");
  });
});

// A page more than three screens away is dropped with its links. One that had the keyboard took it with it, to nothing: the next
// Tab started outside the pane and Esc went to the window (PR review, 2026-11-19).
describe("handKeyboardOn", () => {
  const link = { id: "link" };
  const page = { contains: (n: unknown) => n === link };
  it("a_dropped_page_hands_the_keyboard_on_its_link_to_the_pages", () => {
    const pages = { focus: vi.fn() };
    handKeyboardOn(page, link, pages);
    expect(pages.focus).toHaveBeenCalledWith({ preventScroll: true });
  });
  it("handKeyboardOn_leaves_the_keyboard_that_is_elsewhere", () => {
    const pages = { focus: vi.fn() };
    handKeyboardOn(page, { id: "toolbar" }, pages);
    handKeyboardOn(null, link, pages);
    expect(pages.focus).not.toHaveBeenCalled();
  });
});
