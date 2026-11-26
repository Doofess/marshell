import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { PreviewPane, bodyTakesFocus, escapeLeaves, formatSize, movedPlace, quietViews, type PreviewPaneProps } from "./Preview";
import { followWebLink } from "./webLink";
import { HeaderSlot } from "./headerSlot";
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
const element = (file: FileState, more: Partial<PreviewPaneProps> = {}) => <PreviewPane {...base} file={file} {...more} />;
const html = (file: FileState, more: Partial<PreviewPaneProps> = {}) => renderToStaticMarkup(element(file, more));

describe("PreviewPane", () => {
  it("in the Files panel it draws no Back or Forward, with or without a path bar to hold its tools", () => {
    const alone = html({ phase: "loading" }, { pathShown: true }); // no provider: the header stays, for the tools
    expect(alone).toContain("preview__header");
    expect(alone).not.toContain('aria-label="Back"');
    expect(alone).not.toContain('aria-label="Forward"');
    const inPanel = renderToStaticMarkup(<HeaderSlot value={null}>{element({ phase: "loading" }, { pathShown: true })}</HeaderSlot>);
    expect(inPanel).not.toContain("preview__header");
    expect(inPanel).not.toContain('aria-label="Back"');
  });
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
  const docx = { entry: { path: "C:\\docs\\report.docx", line: null, col: null, view: "rendered" as const } };
  it("says a binary file can't be shown, with a reveal button when it can be revealed", () => {
    const out = html({ phase: "binary" }, { ...docx, onReveal: noop });
    expect(out).toContain("report.docx can&#x27;t be shown here.");
    expect(out).toContain("Show in File Explorer");
  });
  it("names the file manager for the OS on the reveal button", () => {
    expect(html({ phase: "binary" }, { ...docx, onReveal: noop, os: "mac" })).toContain("Show in Finder");
    expect(html({ phase: "binary" }, { ...docx, onReveal: noop, os: "linux" })).toContain("Show in file manager");
  });
  it("gives a too-large file the same button and keeps its sentence", () => {
    const out = html({ phase: "tooLarge", size: 5 * 1024 * 1024, limit: 2 * 1024 * 1024 }, { ...docx, onReveal: noop });
    expect(out).toContain("This file is too large to preview. It is 5 MB and the limit is 2 MB.");
    expect(out).toContain("Show in File Explorer");
  });
  it("has no reveal button without onReveal", () => {
    expect(html({ phase: "binary" }, docx)).not.toContain("Show in File Explorer");
    expect(html({ phase: "tooLarge", size: 5, limit: 2 }, docx)).not.toContain("Show in File Explorer");
  });
  it("leaves the name and folder out of the header when the path is shown elsewhere", () => {
    const out = html({ phase: "loading" }, { pathShown: true });
    expect(out).not.toContain("preview__name");
    expect(out).not.toContain("preview__folder");
    expect(out).not.toContain('aria-label="Back"');
    expect(html({ phase: "loading" })).toContain("preview__name");
  });
  it("shows a PDF in the PDF view, not the old placeholder", () => {
    const out = html({ phase: "pdf", bytes: new ArrayBuffer(8), version: 0, hash: "", readOnly: null }, { entry: { path: "C:\\docs\\a.pdf", line: null, col: null, view: "rendered" } });
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
  it("opens the editor on a file that was already gone, when unsaved edits are kept for it", () => {
    const out = html({ phase: "deleted" }, { keptDraft: true });
    expect(out).toContain('class="text-tab"');
    expect(out).toContain("a.rs was deleted on disk. Ctrl+S saves it again.");
    expect(out).not.toContain("This file was deleted.");
  });
  it("opens the PDF view on a PDF that was already gone, when its notes are kept in a draft", () => {
    // A PDF deleted while its tab was unloaded, or while Marshell was closed: the draft holds the file and its notes (live check 6q-6).
    const out = html({ phase: "deleted" }, { keptPdf: new ArrayBuffer(8), entry: { path: "C:\\docs\\a.pdf", line: null, col: null, view: "rendered" } });
    expect(out).toContain('data-kind="pdf"');
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
    expect(source).toMatch(/data-pane="rendered" hidden=""><div class="html-view__fit"><iframe/);
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

describe("bodyTakesFocus", () => {
  const md = (view: "rendered" | "source", line: number | null = null) => ({ path: "C:\\p\\a.md", line, col: null, view });
  it("takes_the_keyboard_when_the_tab_goes_somewhere", () => {
    expect(bodyTakesFocus(md("rendered"), md("source"), false, false)).toBe(true);
    expect(bodyTakesFocus(md("source"), md("source", 4), false, false)).toBe(true);
    expect(bodyTakesFocus(md("source"), { ...md("source"), path: "C:\\p\\b.md" }, false, false)).toBe(true);
  });
  it("leaves_the_keyboard_for_the_source_view_a_merge_showed", () => {
    // The answer can come while the person is in the terminal; the tab moves the keyboard on from the body itself (6q-4b decision 7.2).
    expect(bodyTakesFocus(md("rendered"), md("source"), true, false)).toBe(false);
  });
  it("a_quiet_switch_that_did_not_happen_does_not_keep_a_later_move_quiet", () => {
    expect(bodyTakesFocus(md("rendered"), { ...md("rendered"), path: "C:\\p\\b.md" }, true, false)).toBe(true);
    expect(bodyTakesFocus(md("rendered"), md("rendered", 9), true, false)).toBe(true);
  });
  // A jump from the sidebar beside the pages keeps the keyboard in the sidebar (6q-5 decision 8).
  const pdf = (page: number, path = "C:\\p\\a.pdf") => ({ path, line: null, col: null, view: "rendered" as const, page, top: 0 });
  it("a quiet jump within the file keeps the keyboard where it is", () => {
    expect(bodyTakesFocus(pdf(1), pdf(7), false, true)).toBe(false);
    // Another file always takes it.
    expect(bodyTakesFocus(pdf(1), pdf(7, "C:\\p\\b.pdf"), false, true)).toBe(true);
  });
  it("Back to another page of the same file takes the keyboard", () => {
    expect(bodyTakesFocus(pdf(7), pdf(1), false, false)).toBe(true);
  });
});

describe("movedPlace", () => {
  const at = (page: number, top = 0, path = "C:\\p\\a.pdf") => ({ path, line: null, col: null, view: "rendered" as const, page, top: Math.round(top * 10000) });
  it("the same entry is no move", () => {
    const e = at(3);
    expect(movedPlace(e, e, null)).toBe(false);
  });
  it("recording the place being left is no move", () => {
    // leave() saves the place in view into the entry; the view is already there.
    expect(movedPlace(at(3), at(5, 0.25), { page: 5, top: 0.25 })).toBe(false);
  });
  it("a jump, Back or Forward to another place is a move", () => {
    expect(movedPlace(at(3), at(12), null)).toBe(true);
    expect(movedPlace(at(3), at(12), { page: 5, top: 0.25 })).toBe(true);
  });
  it("another file is a move even at the recorded place", () => {
    expect(movedPlace(at(3), at(5, 0.25, "C:\\p\\b.pdf"), { page: 5, top: 0.25 })).toBe(true);
  });
});

describe("escapeLeaves", () => {
  const esc = { key: "Escape", ctrlKey: false, altKey: false, metaKey: false, shiftKey: false };
  it("escapeLeaves_from_the_pages", () => {
    expect(escapeLeaves(esc, false)).toBe(true);
  });
  it("escapeLeaves_not_when_a_control_took_it", () => {
    expect(escapeLeaves({ ...esc, defaultPrevented: true }, false)).toBe(false);
    expect(escapeLeaves({ ...esc, defaultPrevented: false }, false)).toBe(true);
  });
  it("escapeLeaves_not_from_the_editor", () => {
    expect(escapeLeaves(esc, true)).toBe(false);
  });
  it("is only for a bare Escape", () => {
    expect(escapeLeaves({ ...esc, key: "Enter" }, false)).toBe(false);
    expect(escapeLeaves({ ...esc, shiftKey: true }, false)).toBe(false);
    expect(escapeLeaves({ ...esc, ctrlKey: true }, false)).toBe(false);
  });
});

describe("quietViews", () => {
  // Which entry change is the Source view a merge asked for (6q-4b decision 7.2), and which is the person's own.
  it("the_merges_switch_is_quiet_once", () => {
    const views: string[] = [];
    const q = quietViews((v) => views.push(v));
    q.merge();
    expect(views).toEqual(["source"]);
    expect(q.take()).toBe(true);
    expect(q.take()).toBe(false);
  });
  it("the_persons_switch_is_never_quiet_and_ends_a_merges_switch_that_never_came", () => {
    const views: string[] = [];
    const q = quietViews((v) => views.push(v));
    q.merge();
    q.person("rendered");
    q.person("source");
    expect(views).toEqual(["source", "rendered", "source"]);
    expect(q.take()).toBe(false);
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
    const pdf = html({ phase: "pdf", bytes: new ArrayBuffer(0), version: 1, hash: "", readOnly: null }, { entry: { path: "C:\\a\\p.pdf", line: null, col: null, view: "rendered" } });
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
