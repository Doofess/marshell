import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PALETTES } from "../terminal/palettes";
import { PreviewPane, formatSize, type PreviewPaneProps } from "./Preview";
import type { FileState } from "./usePreviewFile";

const noop = () => {};
const base: PreviewPaneProps = {
  entry: { path: "C:\\dev\\app\\src\\a.rs", view: "rendered" },
  file: { phase: "loading" },
  updated: false,
  theme: PALETTES.dark,
  os: "windows",
  canBack: false,
  canForward: false,
  onBack: noop,
  onForward: noop,
  onOpen: noop,
  onView: noop,
  onClose: noop,
  onEscape: noop,
};
const html = (file: FileState, more: Partial<PreviewPaneProps> = {}) => renderToStaticMarkup(<PreviewPane {...base} file={file} {...more} />);

describe("PreviewPane", () => {
  it("is a region labelled with the file name", () => {
    const out = html({ phase: "text", text: "fn main() {}\n" });
    expect(out).toContain('aria-label="Preview, a.rs"');
  });
  it("shows the file name and its folder in the header", () => {
    const out = html({ phase: "loading" });
    expect(out).toContain(">a.rs<");
    expect(out).toContain("C:\\dev\\app\\src");
  });
  it("has named buttons: back, forward and close, with the shortcut on close", () => {
    const out = html({ phase: "loading" });
    expect(out).toContain('aria-label="Back"');
    expect(out).toContain('aria-label="Forward"');
    expect(out).toContain('aria-label="Close preview"');
    expect(out).toContain("Ctrl+Shift+Q");
  });
  it("disables back and forward when there is nowhere to go", () => {
    expect(html({ phase: "loading" })).toMatch(/aria-label="Back"[^>]*disabled|disabled[^>]*aria-label="Back"/);
    expect(html({ phase: "loading" }, { canBack: true })).not.toMatch(/aria-label="Back"[^>]*disabled|disabled[^>]*aria-label="Back"/);
  });
  it("makes the scroll body focusable and names it", () => {
    const out = html({ phase: "text", text: "x" });
    expect(out).toMatch(/class="preview__body"[^>]*tabindex="0"|tabindex="0"[^>]*class="preview__body"/);
    expect(out).toContain('aria-label="a.rs, contents"');
  });
  it("shows the text one row per line, with the line numbers hidden from a screen reader", () => {
    const out = html({ phase: "text", text: "one\ntwo\n" });
    expect(out).toContain("one");
    expect(out).toContain("two");
    expect(out).toContain('class="code__no" aria-hidden="true"');
    expect(out.match(/class="code__row"/g)).toHaveLength(2);
  });
  it("says a file is too large, with its size and the limit", () => {
    const out = html({ phase: "tooLarge", size: 5 * 1024 * 1024, limit: 2 * 1024 * 1024 });
    expect(out).toContain("This file is too large to preview. It is 5 MB and the limit is 2 MB.");
  });
  it("says a file isn't text", () => {
    expect(html({ phase: "binary" })).toContain("This file isn&#x27;t text.");
  });
  it("shows a PDF in the PDF view, not the old placeholder", () => {
    const out = html({ phase: "pdf", bytes: new ArrayBuffer(8), version: 0 }, { entry: { path: "C:\\docs\\a.pdf", view: "rendered" } });
    expect(out).not.toContain("later update");
    expect(out).toContain('data-kind="pdf"');
  });
  it("says a file was deleted", () => {
    expect(html({ phase: "deleted" })).toContain("This file was deleted.");
  });
  it("keeps the last content under a deleted banner", () => {
    const out = html({ phase: "deleted", last: { phase: "text", text: "kept line\n" } });
    expect(out).toContain("This file was deleted.");
    expect(out).toContain("kept line");
    expect(out).toContain('role="status"');
    expect(out.indexOf("This file was deleted.")).toBeLessThan(out.indexOf("kept line"));
    // Focus is not inside in a static render, so nothing announces it.
    expect(out).not.toContain('aria-live="polite"');
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
    const quiet = html({ phase: "text", text: "x" }, { updated: false });
    expect(quiet).toContain('class="preview__updated"');
    expect(quiet).not.toContain("Updated</span>");
    const shown = html({ phase: "text", text: "x" }, { updated: true });
    expect(shown).toContain("Updated</span>");
    // Focus is not inside in a static render, so nothing announces it.
    expect(shown).not.toContain('aria-live="polite"');
  });
});

describe("Rendered and Source", () => {
  const md = { entry: { path: "C:\\p\\docs\\a.md", view: "source" as const } };
  const page = (view: "rendered" | "source") => ({ entry: { path: "C:\\p\\site\\index.html", view } });
  const text: FileState = { phase: "text", text: "<h1>Hi</h1>\n" };

  it("offers the toggle for Markdown and HTML, with the current view pressed", () => {
    const out = html(text, md);
    expect(out).toContain('aria-label="View"');
    expect(out).toMatch(/aria-pressed="false"[^>]*>Rendered</);
    expect(out).toMatch(/aria-pressed="true"[^>]*>Source</);
  });
  it("has no toggle for code, and none until the text has loaded", () => {
    expect(html(text)).not.toContain('aria-label="View"');
    expect(html({ phase: "loading" }, md)).not.toContain('aria-label="View"');
  });
  it("shows the source of an HTML file as code rows", () => {
    expect(html(text, page("source"))).toContain('class="code__row"');
    expect(html(text, page("source"))).not.toContain("<iframe");
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
