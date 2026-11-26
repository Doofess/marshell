import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PathBar } from "./PathBar";

const noop = () => {};
const segs = [
  { label: "Airport", folder: "" },
  { label: "src", folder: "src" },
  { label: "b.ts", folder: null },
];
const render = (over: Partial<Parameters<typeof PathBar>[0]> = {}) =>
  renderToStaticMarkup(<PathBar segments={segs} windows treeShown onReveal={noop} onCollapseAll={noop} onToggleTree={noop} onMenu={noop} {...over} />);

describe("PathBar", () => {
  it("is named File path, with a button per folder and plain text for the file", () => {
    const out = render();
    expect(out).toContain('aria-label="File path"');
    expect(out).toMatch(/<button[^>]*>Airport<\/button>/);
    expect(out).toMatch(/<button[^>]*>src<\/button>/);
    expect(out).not.toMatch(/<button[^>]*>b\.ts<\/button>/);
    expect(out).toContain("b.ts");
  });
  it("separators are \\ on Windows and / elsewhere, hidden from the reader", () => {
    expect(render()).toContain('<span aria-hidden="true" class="path-bar__sep">\\</span>');
    expect(render({ windows: false })).toContain('<span aria-hidden="true" class="path-bar__sep">/</span>');
  });
  it("has a place for the file's tools between the path and its own buttons", () => {
    const html = render();
    expect(html.indexOf("</nav>")).toBeLessThan(html.indexOf("path-bar__tools"));
    expect(html.indexOf("path-bar__tools")).toBeLessThan(html.indexOf('aria-label="Collapse all folders"'));
  });
  it("has the two buttons, the toggle pressed while the tree shows", () => {
    const shown = render();
    expect(shown).toContain('aria-label="Collapse all folders"');
    expect(shown).toMatch(/aria-label="Show file tree"[^>]*aria-pressed="true"|aria-pressed="true"[^>]*aria-label="Show file tree"/);
    expect(shown).toContain('title="Hide file tree"');
    expect(shown).toMatch(/<button[^>]*aria-label="Show file tree"[^>]*data-tree-button=""|<button[^>]*data-tree-button=""[^>]*aria-label="Show file tree"/);
    const hidden = render({ treeShown: false });
    expect(hidden).toContain('aria-pressed="false"');
    expect(hidden).toContain('title="Show file tree"');
  });
});
