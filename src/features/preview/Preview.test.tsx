import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { PreviewPane, formatSize, type PreviewPaneProps } from "./Preview";
import { followWebLink } from "./webLink";
import type { FileState } from "./usePreviewFile";

// Rendered Markdown needs the DOM, which these tests run without: the stand-in shows the text it was given.
vi.mock("./MarkdownView", () => ({ MarkdownView: ({ text }: { text: string }) => <div data-md="">{text}</div> }));

const noop = () => {};
const base: PreviewPaneProps = {
  entry: { path: "C:\\dev\\app\\src\\a.rs", line: null, col: null, view: "rendered" },
  file: { phase: "loading" },
  updated: false,
  announce: noop,
  refetch: noop,
  os: "windows",
  canBack: false,
  canForward: false,
  onBack: noop,
  onForward: noop,
  onOpen: noop,
  onView: noop,
  active: true,
  onEscape: noop,
  regionTarget: null,
  onRegion: () => Promise.resolve(),
};
const html = (file: FileState, more: Partial<PreviewPaneProps> = {}) => renderToStaticMarkup(<PreviewPane {...base} file={file} {...more} />);

describe("PreviewPane", () => {
  it("has no region label of its own: the tab panel names it", () => {
    const out = html({ phase: "text", text: "fn main() {}\n", encoding: "utf-8", eol: "lf", hash: "", readOnly: null });
    expect(out).not.toContain('aria-label="Preview, ');
    expect(out).not.toContain("data-drop-preview");
  });
  it("shows the file name and its folder in the header", () => {
    const out = html({ phase: "loading" });
    expect(out).toContain(">a.rs<");
    expect(out).toContain("C:\\dev\\app\\src");
  });
  it("has named back and forward buttons, and no Close preview button", () => {
    const out = html({ phase: "loading" });
    expect(out).toContain('aria-label="Back"');
    expect(out).toContain('aria-label="Forward"');
    expect(out).not.toContain("Close preview");
  });
  it("disables back and forward when there is nowhere to go", () => {
    expect(html({ phase: "loading" })).toMatch(/aria-label="Back"[^>]*disabled|disabled[^>]*aria-label="Back"/);
    expect(html({ phase: "loading" }, { canBack: true })).not.toMatch(/aria-label="Back"[^>]*disabled|disabled[^>]*aria-label="Back"/);
  });
  it("makes the scroll body focusable and names it", () => {
    const out = html({ phase: "text", text: "x", encoding: "utf-8", eol: "lf", hash: "", readOnly: null });
    expect(out).toMatch(/class="preview__body"[^>]*tabindex="0"|tabindex="0"[^>]*class="preview__body"/);
    expect(out).toContain('aria-label="a.rs, contents"');
  });
  it("shows a text file in the editor, with the footer under it", () => {
    const out = html({ phase: "text", text: "one\ntwo\n", encoding: "utf-8", eol: "lf", hash: "", readOnly: null });
    expect(out).toContain('class="text-tab"');
    expect(out).toContain("Ln 1, Col 1");
    expect(out).toContain("Esc, then Tab, leaves the editor");
  });
  it("says a file is too large, with its size and the limit", () => {
    const out = html({ phase: "tooLarge", size: 5 * 1024 * 1024, limit: 2 * 1024 * 1024 });
    expect(out).toContain("This file is too large to preview. It is 5 MB and the limit is 2 MB.");
  });
  it("says a file isn't text", () => {
    expect(html({ phase: "binary" })).toContain("This file isn&#x27;t text.");
  });
  it("shows a PDF in the PDF view, not the old placeholder", () => {
    const out = html({ phase: "pdf", bytes: new ArrayBuffer(8), version: 0 }, { entry: { path: "C:\\docs\\a.pdf", line: null, col: null, view: "rendered" } });
    expect(out).not.toContain("later update");
    expect(out).toContain('data-kind="pdf"');
  });
  it("says a file was deleted", () => {
    expect(html({ phase: "deleted" })).toContain("This file was deleted.");
  });
  it("keeps the last content under a deleted banner", () => {
    const out = html({ phase: "deleted", last: { phase: "image", url: "blob:x" } });
    expect(out).toContain("This file was deleted.");
    expect(out).toContain('src="blob:x"');
    expect(out).toContain('role="status"');
    expect(out.indexOf("This file was deleted.")).toBeLessThan(out.indexOf(`src="blob:x"`));
    // Focus is not inside in a static render, so nothing announces it.
    expect(out).not.toContain('aria-live="polite"');
  });
  it("keeps a deleted text file in its editor, under the notice that says how to save it again", () => {
    const out = html({ phase: "deleted", last: { phase: "text", text: "kept line\n", encoding: "utf-8", eol: "lf", hash: "", readOnly: null } });
    expect(out).toContain("a.rs was deleted on disk. Ctrl+S saves it again.");
    expect(out).toContain('class="text-tab"');
    // The pane's own banner is for the files that have no notice of theirs.
    expect(out).not.toContain("This file was deleted.");
  });
  it("says a file can't be previewed when the core refuses it", () => {
    expect(html({ phase: "error" })).toContain("This file can&#x27;t be previewed.");
  });
  it("shows an image with the file name as its alt text, and its size in the header once known", () => {
    const out = html({ phase: "image", url: "blob:x" });
    expect(out).toContain('src="blob:x"');
    expect(out).toContain('alt="a.rs"');
  });
  it("always renders the Updated note, silent until focus is inside the preview", () => {
    const quiet = html({ phase: "text", text: "x", encoding: "utf-8", eol: "lf", hash: "", readOnly: null }, { updated: false });
    expect(quiet).toContain('class="preview__updated"');
    expect(quiet).not.toContain("Updated</span>");
    const shown = html({ phase: "image", url: "blob:x" }, { updated: true });
    expect(shown).toContain("Updated</span>");
    // Focus is not inside in a static render, so nothing announces it.
    expect(shown).not.toContain('aria-live="polite"');
  });
  it("leaves the Updated note out of a text tab, which says a change through its marks and the announcer", () => {
    const out = html({ phase: "text", text: "x", encoding: "utf-8", eol: "lf", hash: "", readOnly: null }, { updated: true });
    expect(out).not.toContain("Updated</span>");
  });
});

describe("Rendered and Source", () => {
  const md = { entry: { path: "C:\\p\\docs\\a.md", line: null, col: null, view: "source" as const } };
  const page = (view: "rendered" | "source") => ({ entry: { path: "C:\\p\\site\\index.html", line: null, col: null, view } });
  const text: FileState = { phase: "text", text: "<h1>Hi</h1>\n", encoding: "utf-8", eol: "lf", hash: "", readOnly: null };

  it("offers the toggle for Markdown and HTML, with the current view pressed", () => {
    const out = html(text, md);
    expect(out).toContain('aria-label="View"');
    expect(out).toMatch(/<button[^>]*aria-label="Rendered"[^>]*aria-pressed="false"/);
    expect(out).toMatch(/<button[^>]*aria-label="Source"[^>]*aria-pressed="true"/);
  });
  // The user's picture (2026-11-13): an eye for Rendered and </> for Source, each named by its label and its tooltip, no text.
  it("shows each view as an icon named by its label and tooltip", () => {
    const out = html(text, md);
    for (const v of ["Rendered", "Source"]) {
      expect(out, v).toMatch(new RegExp(`<button[^>]*aria-label="${v}"[^>]*title="${v}"[^>]*><svg[^>]*aria-hidden="true"`));
      expect(out, v).not.toContain(`>${v}<`);
    }
  });
  it("has no toggle for code, and none until the text has loaded", () => {
    expect(html(text)).not.toContain('aria-label="View"');
    expect(html({ phase: "loading" }, md)).not.toContain('aria-label="View"');
  });
  it("keeps both views of an HTML file mounted and hides the one that is not chosen", () => {
    const source = html(text, page("source"));
    expect(source).toMatch(/data-pane="rendered" hidden=""><iframe/);
    expect(source).not.toMatch(/data-pane="editor" hidden/);
    const rendered = html(text, page("rendered"));
    expect(rendered).toMatch(/data-pane="editor" hidden=""/);
    expect(rendered).not.toMatch(/data-pane="rendered" hidden/);
  });
  it("renders an HTML file in a frame that is sandboxed with nothing but same-origin", () => {
    const out = html(text, page("rendered"));
    expect(out).toContain('sandbox="allow-same-origin"');
    expect(out).not.toMatch(/allow-(scripts|forms|popups|top-navigation|modals)/);
    expect(out).toContain("Content-Security-Policy");
    // The policy comes before the file's own markup.
    expect(out.indexOf("Content-Security-Policy")).toBeLessThan(out.indexOf("&lt;h1&gt;"));
  });
});

describe("formatSize", () => {
  it("rounds to one decimal and drops a trailing .0", () => {
    expect(formatSize(5 * 1024 * 1024)).toBe("5 MB");
    expect(formatSize(1.5 * 1024 * 1024)).toBe("1.5 MB");
    expect(formatSize(300 * 1024)).toBe("300 KB");
    expect(formatSize(900)).toBe("900 B");
  });
});

describe("PreviewPane: select region", () => {
  it("offers the button on an image and on a PDF, not on Markdown", () => {
    const image = html({ phase: "image", url: "blob:x" }, { entry: { path: "C:\\a\\p.png", line: null, col: null, view: "rendered" } });
    const pdf = html({ phase: "pdf", bytes: new ArrayBuffer(0), version: 1 }, { entry: { path: "C:\\a\\p.pdf", line: null, col: null, view: "rendered" } });
    const md = html({ phase: "text", text: "# hi", encoding: "utf-8", eol: "lf", hash: "", readOnly: null }, { entry: { path: "C:\\a\\p.md", line: null, col: null, view: "source" } });
    expect(image).toContain("Select region");
    expect(image).toContain('title="Select region (R)"');
    // A PDF is offered it once its document has loaded, not while it is loading or when it failed (the first render has none).
    expect(pdf).not.toContain("Select region");
    expect(md).not.toContain("Select region");
  });
});

describe("a web link in a document", () => {
  it("opens a browser tab with no question where browser tabs are available", () => {
    const calls: string[] = [];
    const openPage = (url: string) => {
      calls.push(url);
      return true;
    };
    expect(followWebLink("https://example.com/docs", openPage)).toBe("opened");
    expect(calls).toEqual(["https://example.com/docs"]);
  });
  it("still asks before the system browser where they aren't", () => {
    expect(followWebLink("https://example.com/docs", null)).toBe("ask");
  });
});
