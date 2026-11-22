import { type KeyboardEvent as ReactKeyboardEvent, type ReactNode, type Ref, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import { ContextMenu } from "../../components/ContextMenu/ContextMenu";
import type { TreePosition } from "../../generated/TreePosition";
import type { TreeState } from "../../generated/TreeState";
import type { TreeView } from "../../generated/TreeView";
import type { Os } from "../../lib/keymap";
import { focusPane } from "../pane/focusPane";
import { panelOfPath } from "../pane/panels";
import { cachedFileList, folderName } from "../preview/quickFiles";
import { listTree, searchFiles } from "./filesApi";
import { afterOpen, followTarget, keyboardToFile, openHow, rowOpens } from "./filesActions";
import { FileTree } from "./FileTree";
import { FilesBody } from "./FilesBody";
import { filterMode, namesNote, searchNote, SHORT_NOTE } from "./filterText";
import { PathBar } from "./PathBar";
import { pathSegments } from "./pathSegments";
import { revealFile } from "./reveal";
import { SearchResults } from "./SearchResults";
import { TreeFilter } from "./TreeFilterBox";
import { filterRows } from "./treeFilter";
import { TREE } from "./treeLayout";
import { treeMenuItems, type TreeChoice } from "./treeMenu";
import { absoluteOf, type TreeMove, type TreeRow } from "./treeModel";
import { useSearch } from "./useSearch";
import { useTree, type TreeSource } from "./useTree";

/** What the window can ask of the panel. The panel is mounted while Files is on screen. */
export type FilesPanelHandle = {
  /** Shows or hides the tree (in the narrow layout, the tree over the file). Returns whether it shows now. */
  toggleTree(): boolean;
  collapseAll(): void;
};

export type FilesPanelProps = {
  session: string;
  /** The session's saved tree: its root and open folders. */
  saved: TreeState | undefined;
  /** The window's tree view: hidden, and its width. */
  treeView: TreeView;
  position: TreePosition;
  os: Os;
  /** The user's home folder, so a file under it and outside the project starts the path bar at `~`; null until it is known. */
  home?: string | null;
  /** The path of the Files panel's selected tab, or null with no tab. */
  activePath: string | null;
  /** Where "Paste path into" goes: the session on screen, with the reason it can't take one. */
  pasteTo: { name: string; refused?: string } | null;
  /** The tab body: the kept tabs or the empty state. */
  file: ReactNode;
  onSaved(session: string, tree: TreeState): void;
  onTreeView(view: Partial<TreeView>): void;
  /** Opens a file of the tree (absolute path), as a preview tab unless `keep`. */
  onOpen(o: { path: string; line?: number; keep: boolean }): void;
  onPaste(path: string): void;
  /** Esc in the tree, the results or an empty filter: the keyboard goes back to the terminal. */
  onEscape(): void;
  announce(text: string): void;
  ref?: Ref<FilesPanelHandle>;
};

const BAR_ITEMS: MenuItem[] = [
  { id: "copy-path" satisfies TreeChoice, label: "Copy path" },
  { id: "copy-relative" satisfies TreeChoice, label: "Copy relative path" },
];

type Anchor = { x: number; y: number };

/**
 * The Files panel: the path bar over the open file, and the tree (or a search's results) with its filter box beside it, laid out
 * by `FilesBody`. The tree is the session's: its folders are read by `useTree` and saved with the session; whether it shows and its
 * width are the window's. Opening, pasting and the keyboard's hand-offs go to the window through the props.
 */
export function FilesPanel(p: FilesPanelProps) {
  const { session, treeView, os, activePath, announce } = p;
  const windows = os === "windows";
  const box = useRef<HTMLDivElement>(null);
  const filterBox = useRef<HTMLInputElement>(null);
  const pointer = useRef(false);
  const [size, setSize] = useState({ width: 800, rtl: false });
  const [overlay, setOverlay] = useState(activePath === null);
  const [text, setText] = useState("");
  const [focusedPath, setFocusedPath] = useState<string | null>(null);
  const [filterOpen, setFilterOpen] = useState<ReadonlySet<string>>(new Set());
  const [rowMenu, setRowMenu] = useState<{ row: TreeRow; anchor: Anchor; opener: HTMLElement } | null>(null);
  const [barMenu, setBarMenu] = useState<{ anchor: Anchor; opener: HTMLElement } | null>(null);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setSize((s) => {
      const next = { width: el.clientWidth, rtl: getComputedStyle(el).direction === "rtl" };
      return s.width === next.width && s.rtl === next.rtl ? s : next;
    });
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const narrow = size.width < TREE.narrow;
  const shown = !treeView.hidden && (!narrow || overlay);

  const source = useMemo<TreeSource>(() => ({ list: (asks) => listTree(session, asks) }), [session]);
  const tree = useTree({ session, source, saved: p.saved, drawn: shown, onSaved: p.onSaved });
  const root = tree.root;

  const mode = filterMode(text);
  const search = useSearch({ tab: session, query: mode.kind === "search" ? mode.query : null, run: searchFiles });
  const [names, setNames] = useState<{ files: string[]; truncated: boolean } | "loading" | "error">("loading");
  useEffect(() => {
    if (mode.kind !== "names") return;
    let live = true;
    cachedFileList(session).then(
      (l) => live && setNames({ files: l.files, truncated: l.truncated }),
      () => live && setNames("error"),
    );
    return () => {
      live = false;
    };
  }, [session, mode.kind]);
  const namesQuery = mode.kind === "names" ? mode.query : null;
  const filtered = useMemo(
    () => (namesQuery !== null && typeof names !== "string" ? { ...filterRows(names.files, namesQuery, filterOpen), listCut: names.truncated } : null),
    [namesQuery, names, filterOpen],
  );
  const rows = filtered ? filtered.rows : tree.rows;

  const selected = followTarget(root, activePath, windows);
  // The tree follows the open file: its folders open, its row is marked and comes into view.
  useEffect(() => {
    if (selected === null) return;
    tree.reveal(selected);
    setFocusedPath(selected);
  }, [selected]); // eslint-disable-line react-hooks/exhaustive-deps

  const abs = (rel: string) => (root === null ? rel : absoluteOf(root, rel, windows));
  const sepOf = (rel: string) => (windows ? rel.replace(/\//g, "\\") : rel);

  const toggleTree = (): boolean => {
    if (narrow) {
      const next = treeView.hidden ? true : !overlay;
      if (treeView.hidden) p.onTreeView({ hidden: false });
      setOverlay(next);
      return next;
    }
    p.onTreeView({ hidden: !treeView.hidden });
    return treeView.hidden;
  };
  useImperativeHandle(p.ref, () => ({ toggleTree, collapseAll: () => tree.collapseAll() }));

  /** Opens the file at `rel`, from a tree row, a menu or a search result. */
  const openFile = (rel: string, how: "click" | "enter" | "double" | "space", line?: number) => {
    const path = abs(rel);
    p.onOpen({ path, keep: keyboardToFile(how), ...(line === undefined ? {} : { line }) });
    // An HTML file opens in Browser: the keyboard goes there on Enter, and the tree over the file stays.
    const after = afterOpen(how, { narrow, panel: panelOfPath(path) });
    if (after.close) setOverlay(false);
    if (after.focus !== null) focusPane(after.focus);
  };
  /** Opens a row of the tree. Only a file row opens: a link row is never opened, whichever way the open came. */
  const open = (rel: string, how: "click" | "enter" | "double" | "space") => {
    if (rowOpens(rows.find((r) => r.path === rel))) openFile(rel, how);
  };

  const move = (m: TreeMove) => {
    if (!m) return;
    if (m.type === "focus") {
      const r = rows[m.index];
      if (r) setFocusedPath(r.path);
    } else if (m.type === "expand" || m.type === "collapse") {
      if (filtered) {
        setFilterOpen((s) => {
          const next = new Set(s);
          if (m.type === "expand") next.add(m.path);
          else next.delete(m.path);
          return next;
        });
      } else if (m.type === "expand") tree.expand(m.path);
      else tree.collapse(m.path);
    } else if (m.type === "open") {
      open(m.path, openHow(m.keep, pointer.current));
    } else if (m.type === "filter") {
      filterBox.current?.focus();
      filterBox.current?.select();
    }
  };

  const copy = (value: string, said: string, failed: string) => void navigator.clipboard.writeText(value).then(() => announce(said), () => announce(failed));
  const copyPath = (path: string) => copy(path, "Copied the path.", "Couldn't copy the path.");
  const copyRelative = (rel: string) => copy(sepOf(rel), "Copied the relative path.", "Couldn't copy the relative path.");

  const runRowMenu = (id: string) => {
    const m = rowMenu;
    setRowMenu(null);
    if (!m) return;
    m.opener.focus();
    const choice = id as TreeChoice;
    const path = abs(m.row.path);
    if (choice === "open") open(m.row.path, "enter");
    else if (choice === "paste") p.onPaste(path);
    else if (choice === "copy-path") copyPath(path);
    else if (choice === "copy-relative") copyRelative(m.row.path);
    else if (choice === "reveal") void revealFile(path, announce);
  };
  const runBarMenu = (id: string) => {
    const m = barMenu;
    setBarMenu(null);
    if (!m) return;
    m.opener.focus();
    const target = activePath ?? root;
    if (target === null) return;
    if (id === "copy-path") copyPath(target);
    else if (id === "copy-relative") copyRelative(followTarget(root, target, windows) ?? "");
  };

  const revealFolder = (folder: string) => {
    if (treeView.hidden) p.onTreeView({ hidden: false });
    if (narrow) setOverlay(true);
    if (folder !== "") {
      tree.reveal(folder);
      setFocusedPath(folder);
    }
  };

  // Esc in the tree or the results: the keyboard goes back to the terminal. In the narrow layout FilesBody closes the tree first.
  const onTreeKey = (e: ReactKeyboardEvent<HTMLElement>) => {
    if (e.key !== "Escape" || e.defaultPrevented || narrow) return;
    e.preventDefault();
    e.stopPropagation();
    p.onEscape();
  };
  // Esc in a box with text clears it and only that: FilesBody's Esc (the narrow tree closing) is for the other cases.
  const onFilterKey = (e: ReactKeyboardEvent<HTMLElement>) => {
    if (e.key === "Escape" && text !== "") e.stopPropagation();
  };
  const toRows = () => box.current?.querySelector<HTMLElement>('[role="tree"] [role="treeitem"][tabindex="0"]')?.focus();

  const note =
    mode.kind === "short"
      ? SHORT_NOTE
      : mode.kind === "search"
        ? searchNote(search.reply, mode.query, search.failed)
        : filtered
          ? namesNote({ matches: filtered.matches, capped: filtered.capped, listCut: filtered.listCut }, namesQuery ?? "")
          : null;

  const filter = (
    <div onKeyDown={onFilterKey}>
      <TreeFilter value={text} onChange={setText} onEscape={() => !narrow && p.onEscape()} onDown={toRows} inputRef={filterBox} />
      {note && (
        <p className="file-tree__none" style={{ paddingBlock: "var(--space-2)", paddingInline: "var(--space-3)" }}>
          {note}
        </p>
      )}
    </div>
  );

  const body =
    mode.kind === "search" && !tree.noFolder ? (
      search.reply ? (
        <SearchResults
          reply={search.reply}
          rtl={size.rtl}
          onOpen={(path, line, keep) => openFile(path, openHow(keep, pointer.current), line)}
          onFilter={() => filterBox.current?.focus()}
          announce={announce}
          keyboardHere={() => box.current?.contains(document.activeElement) ?? false}
        />
      ) : null
    ) : (
      <FileTree
        rows={rows}
        rootName={root === null ? "" : folderName(root)}
        noFolder={tree.noFolder}
        selectedPath={selected}
        focusedPath={focusedPath}
        rtl={size.rtl}
        onMove={move}
        onFocusRow={setFocusedPath}
        onMenu={(row, anchor, opener) => setRowMenu({ row, anchor, opener })}
        onDoubleOpen={(path) => open(path, "double")}
      />
    );
  const treeNode = (
    <div style={{ display: "contents" }} onClickCapture={() => (pointer.current = true)} onKeyDownCapture={() => (pointer.current = false)} onKeyDown={onTreeKey}>
      {body}
    </div>
  );

  const segments = pathSegments(root, activePath, { windows, home: p.home ?? null });
  const pathBar = (
    <PathBar
      segments={segments}
      windows={windows}
      treeShown={shown}
      onReveal={revealFolder}
      onCollapseAll={() => tree.collapseAll()}
      onToggleTree={toggleTree}
      onMenu={(anchor, opener) => setBarMenu({ anchor, opener })}
    />
  );
  const position: TreePosition = p.position;
  return (
    <div ref={box} className="files-panel" style={{ display: "grid", blockSize: "100%", minInlineSize: 0, minBlockSize: 0 }}>
      <FilesBody
        tree={treeNode}
        filter={filter}
        file={p.file}
        pathBar={pathBar}
        shown={!treeView.hidden}
        position={position}
        width={treeView.width}
        panelWidth={size.width}
        rtl={size.rtl}
        overlayOpen={overlay}
        onWidth={(w) => p.onTreeView({ width: w })}
        onOverlay={(next) => {
          setOverlay(next);
          // The tree that had the keyboard is gone: it goes to the file.
          if (!next) focusPane("files");
        }}
      />
      {rowMenu && (
        <ContextMenu
          label={`Menu for ${rowMenu.row.name}`}
          items={treeMenuItems(rowMenu.row, { pasteTo: p.pasteTo, os })}
          anchor={rowMenu.anchor}
          onRun={runRowMenu}
          onClose={({ refocus }) => {
            const m = rowMenu;
            setRowMenu(null);
            if (refocus) m.opener.focus();
          }}
        />
      )}
      {barMenu && (
        <ContextMenu
          label="Path menu"
          items={BAR_ITEMS}
          anchor={barMenu.anchor}
          onRun={runBarMenu}
          onClose={({ refocus }) => {
            const m = barMenu;
            setBarMenu(null);
            if (refocus) m.opener.focus();
          }}
        />
      )}
    </div>
  );
}
