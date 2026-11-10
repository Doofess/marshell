import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyPane } from "./EmptyPane";

const noop = () => {};
const html = (recent: string[]) => renderToStaticMarkup(<EmptyPane recent={recent} onOpenFile={noop} onBrowse={noop} onRecent={noop} />);

describe("EmptyPane", () => {
  it("says what is empty and offers the two ways in and the drop hint", () => {
    const out = html([]);
    expect(out).toContain("No tabs open in this session.");
    expect(out).toContain("Open file…");
    expect(out).toContain("Browse…");
    expect(out).toContain("Drop a file here to open it.");
  });
  it("lists recent files as buttons with the name and the folder", () => {
    const out = html(["C:\\work\\app\\README.md"]);
    expect(out).toContain("Recent in this project");
    expect(out).toContain(">README.md<");
    expect(out).toContain(">C:\\work\\app<");
    expect(out).toContain('title="C:\\work\\app\\README.md"');
  });
  it("leaves out the heading with no recent files", () => {
    expect(html([])).not.toContain("Recent in this project");
  });
  it("has no tabindex of its own on the root", () => {
    expect(html([])).not.toContain("tabindex");
  });
});
