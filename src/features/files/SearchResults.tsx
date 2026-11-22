import { type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, useEffect, useRef, useState } from "react";
import type { SearchPart } from "../../generated/SearchPart";
import type { SearchReply } from "../../generated/SearchReply";
import { splitName } from "../pane/tabType";
import { FileIcon, iconKind } from "./fileIcons";
import type { TreeMove, treeKey } from "./treeModel";
import "./FileTree.css";

type KeyLike = Parameters<typeof treeKey>[0];

export type SearchRow =
  | { kind: "file"; path: string; expanded: boolean; count: number }
  | { kind: "line"; path: string; line: number; parts: SearchPart[] };

/** "12 matching lines in 4 files." */
export function searchSaid(reply: SearchReply): string {
  const lines = reply.files.reduce((n, f) => n + f.lines.length, 0);
  if (lines === 0) return "No matches.";
  const files = reply.files.length;
  return `${lines.toLocaleString("en-US")} matching ${lines === 1 ? "line" : "lines"} in ${files.toLocaleString("en-US")} ${files === 1 ? "file" : "files"}.`;
}

/** Every file with its lines, the lines left out of a file in `closed`. */
export function searchRows(reply: SearchReply, closed: ReadonlySet<string>): SearchRow[] {
  const rows: SearchRow[] = [];
  for (const f of reply.files) {
    const expanded = !closed.has(f.path);
    rows.push({ kind: "file", path: f.path, expanded, count: f.lines.length });
    if (expanded) for (const l of f.lines) rows.push({ kind: "line", path: f.path, line: l.line, parts: l.parts });
  }
  return rows;
}

export function searchKey(e: KeyLike, rows: readonly SearchRow[], index: number, rtl: boolean): TreeMove {
  if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "f") return { type: "filter" };
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  const row = rows[index];
  const focus = (i: number): TreeMove => (i >= 0 && i < rows.length && i !== index ? { type: "focus", index: i } : null);
  switch (e.key) {
    case "ArrowDown":
      return focus(index + 1);
    case "ArrowUp":
      return focus(index - 1);
    case "Home":
      return focus(0);
    case "End":
      return focus(rows.length - 1);
    case "ArrowRight":
    case "ArrowLeft": {
      if (!row) return null;
      if ((e.key === "ArrowRight") !== rtl) {
        if (row.kind !== "file") return null;
        if (!row.expanded) return { type: "expand", path: row.path };
        return rows[index + 1]?.kind === "line" ? focus(index + 1) : null;
      }
      if (row.kind === "file") return row.expanded ? { type: "collapse", path: row.path } : null;
      for (let i = index - 1; i >= 0; i--) if (rows[i]?.kind === "file") return focus(i);
      return null;
    }
    case "Enter":
    case " ":
      if (!row) return null;
      if (row.kind === "line") return { type: "open", path: row.path, line: row.line, keep: false };
      return { type: "open", path: row.path, keep: e.key === "Enter" };
    default:
      return null;
  }
}

const rowKey = (r: SearchRow) => (r.kind === "file" ? `f:${r.path}` : `l:${r.path}:${r.line}`);

function Chevron() {
  return (
    <svg className="file-tree__chevron" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m5.5 4 3 3-3 3" />
    </svg>
  );
}

export function SearchResults(p: {
  reply: SearchReply;
  rtl: boolean;
  /** Open `path` at `line` (a preview) or kept. */
  onOpen(path: string, line: number | undefined, keep: boolean): void;
  onFilter?(): void;
  announce(text: string): void;
  /** Whether the box or the results have the keyboard. */
  keyboardHere(): boolean;
}) {
  const { reply, rtl, onOpen } = p;
  const box = useRef<HTMLDivElement>(null);
  const [closed, setClosed] = useState<ReadonlySet<string>>(new Set());
  const [focused, setFocused] = useState<string | null>(null);
  const rows = searchRows(reply, closed);
  const found = focused === null ? -1 : rows.findIndex((r) => rowKey(r) === focused);
  const stop = found >= 0 ? found : 0;

  // A new reply opens every file again and says how much it found, when the box or the results have the keyboard.
  const said = useRef<SearchReply | null>(null);
  useEffect(() => {
    if (said.current === reply) return;
    said.current = reply;
    setClosed(new Set());
    setFocused(null);
    if (p.keyboardHere()) p.announce(searchSaid(reply));
  }, [reply]); // eslint-disable-line react-hooks/exhaustive-deps

  const setOpen = (path: string, open: boolean) =>
    setClosed((c) => {
      const next = new Set(c);
      if (open) next.delete(path);
      else next.add(path);
      return next;
    });

  const focusRow = (i: number) => {
    const r = rows[i];
    if (!r) return;
    setFocused(rowKey(r));
    for (const el of box.current?.querySelectorAll<HTMLElement>('[role="treeitem"]') ?? []) {
      if (el.dataset.key === rowKey(r)) {
        el.focus();
        break;
      }
    }
  };
  const act = (m: TreeMove) => {
    if (!m) return;
    if (m.type === "focus") focusRow(m.index);
    else if (m.type === "expand") setOpen(m.path, true);
    else if (m.type === "collapse") setOpen(m.path, false);
    else if (m.type === "open") onOpen(m.path, m.line, m.keep);
    else if (m.type === "filter") p.onFilter?.();
  };
  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.defaultPrevented) return;
    const m = searchKey(e, rows, stop, rtl);
    if (!m) return;
    e.preventDefault();
    act(m);
  };

  // Sizes and positions among siblings.
  let fileNo = 0;
  let lineNo = 0;
  let linesOfFile = 0;
  return (
    <div ref={box} className="file-tree search-results" role="tree" aria-label="Search results" data-rtl={rtl ? "true" : undefined} onKeyDown={onKeyDown}>
      {rows.map((r, i) => {
        const key = rowKey(r);
        const common = { "data-key": key, role: "treeitem", tabIndex: i === stop ? 0 : -1, onFocus: () => setFocused(key) } as const;
        if (r.kind === "file") {
          fileNo++;
          lineNo = 0;
          linesOfFile = r.count;
          const { head, tail } = splitName(r.path);
          return (
            <div
              key={key}
              {...common}
              className="file-tree__row"
              style={{ "--level": 0 } as CSSProperties}
              title={r.path}
              aria-label={`${r.path}, ${r.count} ${r.count === 1 ? "line" : "lines"}`}
              aria-level={1}
              aria-setsize={reply.files.length}
              aria-posinset={fileNo}
              aria-expanded={r.expanded}
              onClick={() => setOpen(r.path, !r.expanded)}
              onDoubleClick={() => onOpen(r.path, undefined, true)}
            >
              <span className="file-tree__twisty" aria-hidden="true">
                <Chevron />
              </span>
              <FileIcon kind={iconKind(r.path.split("/").pop() ?? r.path, "file", false)} />
              <span className="file-tree__name">
                <span className="file-tree__head">{head}</span>
                {tail !== "" && <span className="file-tree__tail">{tail}</span>}
              </span>
              <span className="search-results__count">{r.count}</span>
            </div>
          );
        }
        lineNo++;
        const text = r.parts.map((x) => x.text).join("");
        return (
          <div
            key={key}
            {...common}
            className="file-tree__row search-results__line"
            style={{ "--level": 1 } as CSSProperties}
            aria-label={`Line ${r.line}: ${text}`}
            aria-level={2}
            aria-setsize={linesOfFile}
            aria-posinset={lineNo}
            onClick={() => onOpen(r.path, r.line, false)}
          >
            <span className="search-results__no">{r.line}</span>
            <span className="search-results__text">
              {r.parts.map((x, j) =>
                x.hit ? (
                  <b key={j} className="file-tree__hit">
                    {x.text}
                  </b>
                ) : (
                  x.text
                ),
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
