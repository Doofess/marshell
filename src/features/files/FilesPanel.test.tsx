import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyFiles } from "../pane/EmptyFiles";
import { SidePane } from "../pane/SidePane";
import { readFileSync } from "node:fs";
import { barItems, FilesPanel, type FilesPanelProps } from "./FilesPanel";

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
    expect(render()).not.toContain("There is no folder the tree can show.");
  });
  it("a file under the home folder and outside the project starts the path bar at ~ (decision 7)", () => {
    const out = render({ activePath: "/home/me/notes/todo.md", home: "/home/me" });
    expect(out).toMatch(/>~</);
    expect(out).not.toMatch(/>home</);
  });
  it("the strip shows the Files menu button", () => {
    const out = renderToStaticMarkup(
      <SidePane panel="files" tabs={[]} activeId={null} plus={{ label: "Open a file", menu: true }} menu={{ label: "Files menu" }} expand={{ on: false, others: true }} hideKeys="Ctrl+Shift+E" onExpand={noop} onSelect={noop} onClose={noop} onMove={noop} onHide={noop} onPlus={noop}>
        <i />
      </SidePane>,
    );
    expect(out).toContain('aria-label="Files menu"');
    expect(out).toContain('aria-haspopup="menu"');
  });
});

describe("the path bar's menu", () => {
  // Review 6r-2: with no file inside the project folder there is no relative path, and the menu said it had copied one.
  it("offers Copy relative path only for a file inside the project folder", () => {
    const copyPath = { id: "copy-path", label: "Copy path" };
    expect(barItems("src/a.ts")).toEqual([copyPath, { id: "copy-relative", label: "Copy relative path" }]);
    expect(barItems(null)).toEqual([copyPath]);
  });
});

describe("rows listed for an older root", () => {
  const panel = readFileSync(new URL("./FilesPanel.tsx", import.meta.url), "utf8");
  const search = readFileSync(new URL("./useSearch.ts", import.meta.url), "utf8");
  // Review 6r-2: after a `cd` the name list and a search's results were opened against the tree's new root.
  it("open against the root they were listed under, and are listed again for the new one", () => {
    expect(panel).toContain("fileListFor(session, root)");
    expect(panel).toContain("}, [session, mode.kind, root]);");
    expect(panel).toContain("const base = filtered && typeof names !== \"string\" ? names.root : root;");
    expect(panel).toContain("openAt(absoluteOf(reply.root, path, windows)");
    expect(panel).toContain("useSearch({ tab: session, root, query:");
    expect(search).toContain("}, [o.tab, o.root, o.query, o.run]);");
  });
  it("the bar's menu copies a relative path only when there is one", () => {
    expect(panel).toContain("items={barItems(");
    expect(panel).not.toContain('copyRelative(followTarget(root, target, windows) ?? "")');
    expect(panel).toContain('id === "copy-relative" && selected !== null');
  });
});

describe("what Files waits for", () => {
  const panel = readFileSync(new URL("./FilesPanel.tsx", import.meta.url), "utf8");
  it("hides the old note and the old rows at once while Files waits, and draws the ring after 150 ms", () => {
    expect(panel).toContain("filesWait({ mode, names: namesState, searchBusy })");
    expect(panel).toContain('<div className="files-wait" aria-busy="true"');
    expect(panel).toContain("useShownAfter(wait !== null)");
    expect(panel).toContain("<FilesBusy text={wait} />");
    expect(panel).toContain("wait !== null");
    expect(panel).toContain("LONG_NOTE");
    expect(panel).toContain('mode.kind === "search" ? mode.query : null');
  });
  it("says a failed name list in the palette's words and keeps the tree", () => {
    expect(panel).toContain('namesState === "error"');
    expect(panel).toContain("listFailed(root === null ? null : folderName(root))");
  });
  it("moves focus from the rows to the filter box when the wait replaces them", () => {
    expect(panel).toContain("onFocusCapture");
    expect(panel).toContain("onBlurCapture");
    expect(panel).toContain("focusToFilter({ waitBegan");
    expect(panel).toContain("filterBox.current?.focus()");
  });
  it("lists the names again when the root moves and starts from the cached list", () => {
    expect(panel).toContain('setNames("loading")');
    expect(panel).toContain("cachedFileListNow(session, root)");
  });
  it("announces Collapsed all folders. from the menu and the handle", () => {
    expect(panel).toContain("announce(COLLAPSED)");
    expect(panel).toContain("toggleTree, collapseAll, showFile }));");
    expect(panel).toContain("onCollapseAll={collapseAll}");
  });
  it("the handle puts the file in front of the narrow tree (issue #205)", () => {
    expect(panel).toContain("const showFile = () => setOverlay(false);");
  });
});
