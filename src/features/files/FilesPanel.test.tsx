import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyFiles } from "../pane/EmptyFiles";
import { SidePane } from "../pane/SidePane";
import { FilesPanel, type FilesPanelProps } from "./FilesPanel";

const noop = () => {};
const base: FilesPanelProps = {
  session: "t1",
  saved: { root: "/p", expanded: [] },
  treeView: { hidden: false, width: null },
  position: "left",
  os: "linux",
  activePath: "/p/src/a.ts",
  pasteTo: { name: "api-server" },
  file: <i>FILE</i>,
  onSaved: noop,
  onTreeView: noop,
  onOpen: noop,
  onPaste: noop,
  onEscape: noop,
  announce: noop,
};
const render = (over: Partial<FilesPanelProps> = {}) => renderToStaticMarkup(<FilesPanel {...base} {...over} />);

describe("FilesPanel", () => {
  it("draws the path bar, the filter and the tree with the file beside it", () => {
    const out = render();
    expect(out).toContain('aria-label="File path"');
    expect(out).toContain('aria-label="Filter files"');
    expect(out).toContain('role="tree"');
    expect(out).toContain("FILE");
    // The tree is on the left: before the file in the reading order.
    expect(out.indexOf('role="tree"')).toBeLessThan(out.indexOf("FILE"));
  });
  it("the path bar's Show file tree button is pressed while the tree shows", () => {
    expect(render()).toContain('aria-pressed="true"');
    const hidden = render({ treeView: { hidden: true, width: null } });
    expect(hidden).toContain('aria-pressed="false"');
    expect(hidden).not.toContain('role="tree"');
    expect(hidden).toContain("FILE");
  });
  it("a session with the tree and no tab shows the tree beside the empty state", () => {
    const out = render({ activePath: null, file: <EmptyFiles recent={[]} onOpenFile={noop} onBrowse={noop} onRecent={noop} /> });
    expect(out).toContain('role="tree"');
    expect(out).toContain("No files open");
  });
  it("the tree says when the session has no folder it can show only after the core said so", () => {
    expect(render()).not.toContain("This session has no folder the tree can show.");
  });
  it("a file under the home folder and outside the project starts the path bar at ~ (decision 7)", () => {
    const out = render({ activePath: "/home/me/notes/todo.md", home: "/home/me" });
    expect(out).toMatch(/>~</);
    expect(out).not.toMatch(/>home</);
  });
  it("the strip shows the Files menu button", () => {
    const out = renderToStaticMarkup(
      <SidePane panel="files" tabs={[]} activeId={null} plus={{ label: "Open a file", menu: true }} menu={{ label: "Files menu" }} fill={{ on: false }} hideKeys="Ctrl+Shift+E" onFill={noop} onSelect={noop} onClose={noop} onMove={noop} onHide={noop} onPlus={noop}>
        <i />
      </SidePane>,
    );
    expect(out).toContain('aria-label="Files menu"');
    expect(out).toContain('aria-haspopup="menu"');
  });
});
