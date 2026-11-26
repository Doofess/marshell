import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FilesBody } from "./FilesBody";

const noop = () => {};
const render = (over: Partial<Parameters<typeof FilesBody>[0]> = {}) =>
  renderToStaticMarkup(
    <FilesBody
      tree={<i>TREE</i>}
      filter={<i>FILTER</i>}
      file={<i>FILE</i>}
      pathBar={<div className="path-bar" />}
      shown
      position="left"
      width={null}
      panelWidth={800}
      rtl={false}
      overlayOpen={false}
      onWidth={noop}
      onOverlay={noop}
      {...over}
    />,
  );

describe("FilesBody", () => {
  it("the divider says its width", () => {
    const out = render({ width: 300 });
    expect(out).toContain('role="separator"');
    expect(out).toContain('aria-orientation="vertical"');
    expect(out).toContain('aria-valuenow="300"');
    expect(out).toContain('aria-valuemin="160"');
    expect(out).toContain('aria-valuemax="560"');
    expect(out).toContain('aria-label="Resize the file tree"');
    expect(out).toContain('tabindex="0"');
  });
  it("the path bar is the first row and the main row holds the tree, the divider and the file", () => {
    const html = render({ panelWidth: 800, position: "left" });
    expect(html.indexOf('class="path-bar"')).toBeLessThan(html.indexOf("files-body__main"));
    const main = html.slice(html.indexOf("files-body__main"));
    expect(main.indexOf("files-body__tree")).toBeLessThan(main.indexOf("files-body__divider"));
    expect(main.indexOf("files-body__divider")).toBeLessThan(main.indexOf("files-body__file"));
    expect(main).not.toContain("path-bar");
  });
  it("a tree on the end edge comes after the file", () => {
    const main = render({ panelWidth: 800, position: "right" }).split("files-body__main")[1]!;
    expect(main.indexOf("files-body__file")).toBeLessThan(main.indexOf("files-body__tree"));
    // Right to left, a row reads from the right: a tree on the left comes after the file, and one on the right before it.
    const rtlLeft = render({ position: "left", rtl: true }).split("files-body__main")[1]!;
    expect(rtlLeft.indexOf("FILE")).toBeLessThan(rtlLeft.indexOf("TREE"));
    const rtlRight = render({ position: "right", rtl: true }).split("files-body__main")[1]!;
    expect(rtlRight.indexOf("TREE")).toBeLessThan(rtlRight.indexOf("FILE"));
  });
  it("says whether the tree is drawn", () => {
    expect(render({ panelWidth: 800, shown: true })).toContain('data-tree=""');
    expect(render({ panelWidth: 800, shown: false })).not.toContain("data-tree");
    expect(render({ panelWidth: 800, shown: false })).toContain('class="path-bar"');
  });
  it("under 480 px the tree takes the file's place: full width, the file inert under it, no divider", () => {
    const html = render({ panelWidth: 400, overlayOpen: true });
    expect(html).toContain('data-narrow=""');
    expect(html).not.toContain("files-body__divider");
    expect(html).toMatch(/class="files-body__file" inert=""/);
    expect(html).toMatch(/class="files-body__tree"(?! style)/);
    expect(html.indexOf("files-body__file")).toBeLessThan(html.indexOf("files-body__tree"));
  });
  it("under 480 px with the tree closed the file is alone and can be used", () => {
    const html = render({ panelWidth: 400, overlayOpen: false });
    expect(html).not.toContain("files-body__tree");
    expect(html).not.toContain("inert");
  });
  it("a wide panel never makes the file inert", () => {
    expect(render({ panelWidth: 800 })).not.toContain("inert");
  });
});
