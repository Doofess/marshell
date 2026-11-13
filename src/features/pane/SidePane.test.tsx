import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PreviewPane, type PreviewPaneProps } from "../preview/Preview";
import { PALETTES } from "../terminal/palettes";
import { SidePane, type SidePaneProps, type StripTab } from "./SidePane";

const noop = () => {};
const tabs: StripTab[] = [
  { id: "t1", title: "a.md", path: "C:\\w\\a.md", kind: "text" },
  { id: "t2", title: "b.rs", path: "C:\\w\\b.rs", kind: "text" },
];
const html = (more: Partial<SidePaneProps> = {}) =>
  renderToStaticMarkup(
    <SidePane tabs={tabs} activeId="t2" os="windows" onSelect={noop} onClose={noop} onMove={noop} onHide={noop} onPlus={noop} {...more}>
      {more.children ?? <p>body</p>}
    </SidePane>,
  );

describe("SidePane", () => {
  it("is the side pane's section, a drop target and a navigation stop", () => {
    const out = html();
    expect(out).toContain("<section");
    expect(out).toContain('aria-label="Side pane"');
    expect(out).toContain("data-nav-side");
    expect(out).toContain("data-drop-pane");
    expect(out).toContain("<p>body</p>");
  });
  it("has a tablist of tabs wired to their panels, the path as the title", () => {
    const out = html();
    expect(out).toContain('role="tablist"');
    expect(out).toContain('aria-label="Tabs"');
    expect(out.match(/<button[^>]*role="tab"/g)).toHaveLength(2);
    expect(out).toContain('id="pane-tab-t1"');
    expect(out).toContain('aria-controls="pane-panel-t1"');
    expect(out).toContain('title="C:\\w\\b.rs"');
    expect(out).toMatch(/id="pane-tab-t2"[^>]*aria-selected="true"|aria-selected="true"[^>]*id="pane-tab-t2"/);
  });
  it("makes only the active tab a tab stop", () => {
    const out = html();
    const own = (id: string) => out.match(new RegExp(`<button[^>]*id="pane-tab-${id}"[^>]*>`))![0];
    expect(own("t2")).toContain('tabindex="0"');
    expect(own("t1")).toContain('tabindex="-1"');
  });
  it("gives each tab a sibling close button outside the tab", () => {
    const out = html();
    expect(out).toMatch(/<button[^>]*aria-label="Close a\.md"[^>]*>/);
    expect(out.match(/<button[^>]*aria-label="Close a\.md"[^>]*>/)![0]).toContain('tabindex="-1"');
    expect(out).not.toMatch(/<button[^>]*role="tab"[^>]*>(?:(?!<\/button>).)*<button/);
  });
  it("ends with New tab and Hide, the hide title holding the shortcut", () => {
    const out = html();
    expect(out).toMatch(/<button[^>]*aria-label="New tab"[^>]*aria-haspopup="menu"|<button[^>]*aria-haspopup="menu"[^>]*aria-label="New tab"/);
    expect(out).toMatch(/aria-label="Hide the side pane" title="Hide the side pane \(Ctrl\+Shift\+Q\)"/);
    expect(out.indexOf('aria-label="Close b.rs"')).toBeLessThan(out.indexOf('aria-label="New tab"'));
  });
  it("a web tab shows the globe and never an image", () => {
    const globe = html({ tabs: [{ id: "b1", title: "Example.com", path: "https://example.com/", kind: "web" }], activeId: "b1" });
    expect(globe).not.toContain("<img");
    expect(globe).toMatch(/<svg[^>]*class="side-pane__icon"[^>]*data-kind="web"[^>]*aria-hidden="true"|<svg(?=[^>]*data-kind="web")(?=[^>]*class="side-pane__icon")(?=[^>]*aria-hidden="true")/);
    // The icon sits inside the tab button, before the title.
    expect(globe.indexOf("side-pane__icon")).toBeLessThan(globe.indexOf("side-pane__title"));
    expect(globe.indexOf('role="tab"')).toBeLessThan(globe.indexOf("side-pane__icon"));
    // A web title shortens at its end: it is not split.
    expect(globe).not.toContain("side-pane__name-head");
    expect(globe).toContain('<span class="side-pane__title">Example.com</span>');
  });
  it("text, pdf and image tabs each show their own icon", () => {
    for (const kind of ["text", "pdf", "image"] as const) {
      const out = html({ tabs: [{ id: "f1", title: "a.x", path: "a.x", kind }], activeId: "f1" });
      expect(out).toMatch(new RegExp(`<svg(?=[^>]*data-kind="${kind}")(?=[^>]*class="side-pane__icon")`));
      expect(out).not.toContain("<img");
    }
  });
  it("a file tab's name is a head that shortens and a tail that keeps its extension", () => {
    const out = html({ tabs: [{ id: "f1", title: "sample-long-name.txt", path: "C:\\w\\sample-long-name.txt", kind: "text" }], activeId: "f1" });
    expect(out).toContain('<span class="side-pane__name-head">sample-long-name</span><span class="side-pane__name-tail">.txt</span>');
  });
  it("with no tabs shows no tablist, and still the two buttons", () => {
    const out = html({ tabs: [], activeId: null });
    expect(out).not.toContain('role="tablist"');
    expect(out).toContain('aria-label="New tab"');
    expect(out).toContain('aria-label="Hide the side pane"');
  });

  it("a tab whose file can't be read still shows in the strip and closes", () => {
    const pane: PreviewPaneProps = {
      entry: { path: "C:\\w\\a.md", line: null, col: null, view: "rendered" },
      file: { phase: "error" },
      updated: false,
      theme: PALETTES.dark,
      os: "windows",
      active: true,
      canBack: false,
      canForward: false,
      onBack: noop,
      onForward: noop,
      onOpen: noop,
      onView: noop,
      onEscape: noop,
      regionTarget: null,
      onRegion: () => Promise.resolve(),
    };
    const out = html({ tabs: [tabs[0]!], activeId: "t1", children: <PreviewPane {...pane} /> });
    expect(out).toContain('id="pane-tab-t1"');
    expect(out).toContain('aria-label="Close a.md"');
    expect(out).toContain("This file can&#x27;t be previewed.");
  });
});
