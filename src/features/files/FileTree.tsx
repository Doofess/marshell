import { type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import { splitName } from "../pane/tabType";
import { FileIcon, iconKind } from "./fileIcons";
import { type TreeMove, type TreeRow, typeAhead, treeKey } from "./treeModel";
import "./FileTree.css";

/** A row is 24 px tall; FileTree.css says the same. */
const ROW = 24;
/** Rows drawn beyond the ones in view, above and below. */
const OVERSCAN = 10;
/** A letter typed within this long of the last extends the type-ahead. */
const TYPE_AHEAD_MS = 500;
/** A key move focuses the new row only if the row appears within this long of the key. */
const KEY_FOCUS_MS = 200;

/** The rows to draw: those in view plus 10 each way, and `extra`, the row with the keyboard, when it is outside them. */
export function visibleRange(o: { count: number; scrollTop: number; viewport: number; focused: number }): { start: number; end: number; extra: number | null } {
  if (o.count === 0) return { start: 0, end: 0, extra: null };
  const start = Math.max(0, Math.floor(o.scrollTop / ROW) - OVERSCAN);
  const end = Math.min(o.count, Math.ceil((o.scrollTop + o.viewport) / ROW) + OVERSCAN);
  const extra = o.focused >= 0 && o.focused < o.count && (o.focused < start || o.focused >= end) ? o.focused : null;
  return { start, end, extra };
}

/** The letters typed so far: extended when the last one was within 500 ms, started over otherwise. */
export function extendTyped(prev: { text: string; at: number }, key: string, now: number): { text: string; at: number } {
  return { text: prev.text !== "" && now - prev.at <= TYPE_AHEAD_MS ? prev.text + key : key, at: now };
}

function Chevron() {
  return (
    <svg className="file-tree__chevron" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m5.5 4 3 3-3 3" />
    </svg>
  );
}

function RowName({ row }: { row: TreeRow }) {
  if (row.parts) {
    return (
      <span className="file-tree__name">
        <span className="file-tree__head">
          {row.parts.map((p, i) =>
            p.hit ? (
              <b key={i} className="file-tree__hit">
                {p.text}
              </b>
            ) : (
              p.text
            ),
          )}
        </span>
      </span>
    );
  }
  if (row.kind === "folder") {
    return (
      <span className="file-tree__name">
        <span className="file-tree__head">{row.name}</span>
      </span>
    );
  }
  const { head, tail } = splitName(row.name);
  return (
    <span className="file-tree__name">
      <span className="file-tree__head">{head}</span>
      {tail !== "" && <span className="file-tree__tail">{tail}</span>}
    </span>
  );
}

export function FileTree(p: {
  rows: TreeRow[];
  rootName: string;
  noFolder: boolean;
  selectedPath: string | null;
  focusedPath: string | null;
  rtl: boolean;
  onMove(m: TreeMove): void;
  onFocusRow(path: string): void;
  onMenu(row: TreeRow, anchor: { x: number; y: number }, opener: HTMLElement): void;
  onDoubleOpen(path: string): void;
}) {
  const { rows, rtl, onMove, onFocusRow, onMenu } = p;
  const box = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const keyed = useRef(0);
  const [view, setView] = useState({ top: 0, height: 480 });

  const found = p.focusedPath === null ? -1 : rows.findIndex((r) => r.path === p.focusedPath);
  /** The one tab stop: the row with the keyboard, or the first. */
  const stop = found >= 0 ? found : 0;

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setView((v) => (v.height === el.clientHeight ? v : { ...v, height: el.clientHeight }));
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [p.noFolder]);

  // The row with the keyboard comes into view; after a key move it also takes the focus.
  useEffect(() => {
    const el = box.current;
    if (!el || found < 0) return;
    const top = found * ROW;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight;
    if (Date.now() - keyed.current < KEY_FOCUS_MS) {
      keyed.current = 0;
      rowElement(el, p.focusedPath)?.focus({ preventScroll: true });
    }
  }, [p.focusedPath, found]);

  if (p.noFolder) {
    return (
      <div className="file-tree file-tree--none">
        <p className="file-tree__none">This session has no folder the tree can show.</p>
      </div>
    );
  }

  const onScroll = () => {
    const el = box.current;
    if (!el) return;
    const top = Math.floor(el.scrollTop / ROW) * ROW;
    setView((v) => (v.top === top ? v : { ...v, top }));
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.defaultPrevented) return;
    const row = rows[stop];
    const move = treeKey(e, rows, stop, rtl);
    if (move?.type === "menu") {
      e.preventDefault();
      const el = box.current && row ? rowElement(box.current, row.path) : null;
      if (row && el && row.kind !== "more" && row.kind !== "empty") {
        const r = el.getBoundingClientRect();
        onMenu(row, { x: rtl ? r.right - ROW : r.left + ROW, y: r.bottom }, el);
      }
      return;
    }
    if (move) {
      e.preventDefault();
      typed.current = { text: "", at: 0 };
      if (move.type === "focus") keyed.current = Date.now();
      onMove(move);
      return;
    }
    if (e.key.length === 1 && e.key !== " " && !e.ctrlKey && !e.altKey && !e.metaKey) {
      const next = extendTyped(typed.current, e.key, Date.now());
      typed.current = next;
      const i = typeAhead(rows, stop, next.text);
      if (i !== null) {
        e.preventDefault();
        keyed.current = Date.now();
        onMove({ type: "focus", index: i });
      }
    }
  };

  const real = (r: TreeRow) => r.kind !== "more" && r.kind !== "empty";
  const click = (r: TreeRow) => {
    if (!real(r)) return;
    onFocusRow(r.path);
    if (r.kind === "folder") onMove({ type: r.expanded ? "collapse" : "expand", path: r.path });
    else onMove({ type: "open", path: r.path, keep: false });
  };
  const context = (r: TreeRow, e: ReactMouseEvent<HTMLElement>) => {
    if (!real(r)) return;
    e.preventDefault();
    onFocusRow(r.path);
    onMenu(r, { x: e.clientX, y: e.clientY }, e.currentTarget);
  };

  const range = visibleRange({ count: rows.length, scrollTop: view.top, viewport: view.height, focused: stop });
  const spans: [number, number][] = range.extra === null ? [[range.start, range.end]] : range.extra < range.start ? [[range.extra, range.extra + 1], [range.start, range.end]] : [[range.start, range.end], [range.extra, range.extra + 1]];
  const out: ReactNode[] = [];
  let cursor = 0;
  const spacer = (n: number, key: string) => out.push(<div key={key} className="file-tree__spacer" style={{ blockSize: n * ROW }} aria-hidden="true" />);
  for (const [a, b] of spans) {
    if (a > cursor) spacer(a - cursor, `s${a}`);
    for (let i = a; i < b; i++) {
      const row = rows[i]!;
      out.push(<Row key={row.path} row={row} tabStop={i === stop} selected={real(row) && row.path === p.selectedPath} onClick={click} onContext={context} onDouble={p.onDoubleOpen} onFocusRow={onFocusRow} focusedPath={p.focusedPath} />);
    }
    cursor = b;
  }
  if (rows.length > cursor) spacer(rows.length - cursor, "end");

  return (
    <div ref={box} className="file-tree" role="tree" aria-label={`Files in ${p.rootName}`} data-rtl={rtl ? "true" : undefined} onScroll={onScroll} onKeyDown={onKeyDown}>
      {out}
    </div>
  );
}

function rowElement(box: HTMLElement, path: string | null): HTMLElement | null {
  if (path === null) return null;
  for (const el of box.querySelectorAll<HTMLElement>('[role="treeitem"]')) if (el.dataset.path === path) return el;
  return null;
}

function Row(p: {
  row: TreeRow;
  tabStop: boolean;
  selected: boolean;
  focusedPath: string | null;
  onClick(r: TreeRow): void;
  onContext(r: TreeRow, e: ReactMouseEvent<HTMLElement>): void;
  onDouble(path: string): void;
  onFocusRow(path: string): void;
}) {
  const { row } = p;
  const style = { "--level": row.level } as CSSProperties;
  if (row.kind === "more" || row.kind === "empty") {
    return (
      <div role="treeitem" className="file-tree__row file-tree__row--note" data-path={row.path} aria-disabled="true" aria-selected={false} aria-level={row.level + 1} aria-setsize={row.setSize} aria-posinset={row.posInSet} tabIndex={p.tabStop ? 0 : -1} style={style}>
        <span className="file-tree__twisty" aria-hidden="true" />
        <span className="file-tree__name">{row.kind === "more" ? `And ${(row.more ?? 0).toLocaleString("en-US")} more` : "Empty"}</span>
      </div>
    );
  }
  const label = `${row.name}${row.ignored ? ", ignored" : ""}${row.kind === "link" ? ", link" : ""}`;
  const folder = row.kind === "folder";
  return (
    <div
      role="treeitem"
      {...selectedProps(p.selected, "selected", "file-tree__row")}
      data-path={row.path}
      data-kind={row.kind}
      data-ignored={row.ignored ? "true" : undefined}
      aria-label={label}
      aria-level={row.level + 1}
      aria-setsize={row.setSize}
      aria-posinset={row.posInSet}
      aria-expanded={folder ? row.expanded : undefined}
      aria-busy={row.loading ? true : undefined}
      tabIndex={p.tabStop ? 0 : -1}
      title={row.path}
      style={style}
      onClick={() => p.onClick(row)}
      onDoubleClick={() => (row.kind === "file" ? p.onDouble(row.path) : undefined)}
      onContextMenu={(e) => p.onContext(row, e)}
      onFocus={() => {
        if (row.path !== p.focusedPath) p.onFocusRow(row.path);
      }}
    >
      <span className="file-tree__twisty" aria-hidden="true">
        {folder && <Chevron />}
      </span>
      <FileIcon kind={iconKind(row.name, row.kind, row.expanded)} />
      <RowName row={row} />
    </div>
  );
}
