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
      pathBar={<i>BAR</i>}
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
  it("the DOM order follows the reading order", () => {
    const left = render({ position: "left" });
    expect(left.indexOf("TREE")).toBeLessThan(left.indexOf("FILE"));
    const right = render({ position: "right" });
    expect(right.indexOf("FILE")).toBeLessThan(right.indexOf("TREE"));
    // Right to left, a row reads from the right: a tree on the left comes after the file, and one on the right before it.
    const rtlLeft = render({ position: "left", rtl: true });
    expect(rtlLeft.indexOf("FILE")).toBeLessThan(rtlLeft.indexOf("TREE"));
    const rtlRight = render({ position: "right", rtl: true });
    expect(rtlRight.indexOf("TREE")).toBeLessThan(rtlRight.indexOf("FILE"));
  });
  it("under 480 px the tree is drawn over the file", () => {
    const open = render({ panelWidth: 400, overlayOpen: true });
    expect(open).toContain("data-narrow");
    expect(open).toContain("TREE");
    expect(open).not.toContain('role="separator"');
    const closed = render({ panelWidth: 400, overlayOpen: false });
    expect(closed).toContain("data-narrow");
    expect(closed).not.toContain("TREE");
    expect(closed).toContain("FILE");
  });
  it("a hidden tree leaves the file and the bar alone", () => {
    const out = render({ shown: false });
    expect(out).not.toContain("TREE");
    expect(out).not.toContain('role="separator"');
    expect(out).toContain("BAR");
  });
  it("the narrow tree is capped 40 px short of the panel", () => {
    expect(render({ panelWidth: 400, overlayOpen: true, width: 900 })).toContain("inline-size:360px");
  });
});
