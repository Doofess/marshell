import { type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent, type ReactNode, useEffect, useRef } from "react";
import { selectedProps } from "../../components/a11y/SelectedMark";
import type { LineTokens } from "../preview/highlight";
import { copy } from "./copy";
import { type Item, type Row, type Section, itemLabel } from "./model";
import "./ChangesPanel.css";

export type ItemRange = { from: number; to: number };
export type ListResult = {
  focus: number;
  fold?: string;
  unfold?: string;
  range?: ItemRange | null;
  open?: "comment" | "menu";
  escape?: true;
};

/** Rows drawn in one block; each block and each file is skipped by the browser while it is off screen. */
const CHUNK = 200;
/** A key move focuses the new item only if the item appears within this long of the key. */
const KEY_FOCUS_MS = 200;

const pathOf = (item: Item): string => (item.kind === "comment" ? item.comment.path : item.file.path);
const fileIndexOf = (items: Item[], i: number): number => {
  const path = pathOf(items[i]!);
  return items.findIndex((x) => x.kind === "file" && x.file.path === path);
};
const rowOf = (item: Item | undefined): Row | undefined => (item?.kind === "row" ? item.file.rows[item.at] : undefined);
/** A row a comment can start on or a range can hold: any row but a hunk's header. */
const takesRange = (item: Item | undefined): boolean => {
  const row = rowOf(item);
  return row !== undefined && row.kind !== "hunk";
};

/** The next row item in the same file, `dir` away from `from`, passing comments and notes; null at the file's edge. */
function stepRow(items: Item[], from: number, dir: 1 | -1): number | null {
  for (let i = from + dir; i >= 0 && i < items.length; i += dir) {
    const item = items[i]!;
    if (item.kind === "row") return i;
    if (item.kind === "file" || item.kind === "show") return null;
  }
  return null;
}

/**
 * The item a range reaches when it is widened from `anchor` toward `to`: it never passes a hunk's header, the end of its file or a
 * row that isn't one. Returns `to` when nothing stops it, else the last item before the stop.
 */
export function reach(items: Item[], anchor: number, to: number): number {
  if (!takesRange(items[anchor])) return anchor;
  const dir = to >= anchor ? 1 : -1;
  let at = anchor;
  while (at !== to) {
    const next = stepRow(items, at, dir);
    if (next === null || !takesRange(items[next]) || (dir === 1 ? next > to : next < to)) return at;
    at = next;
  }
  return at;
}

const LIST_KEYS = new Set(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "Enter", "Escape", "ContextMenu"]);

/** Whether the list takes this key (Alt works only with Up and Down; F10 only with Shift). */
export function isListKey(key: string, mods: { shift: boolean; alt: boolean }): boolean {
  if (key === "F10") return mods.shift && !mods.alt;
  if (!LIST_KEYS.has(key)) return false;
  if (mods.alt) return key === "ArrowUp" || key === "ArrowDown";
  return true;
}

/**
 * What a key does in the diff list, as data: the item the keyboard goes to and, when it applies, a file to fold or unfold, the new
 * range (null drops it), a box to open, or leaving the list. `items` is `itemsOf`'s list, `focus` an index in it and `range` the
 * selected rows (`from` is where it started, `to` where it ends and the keyboard is).
 */
export function listKey(items: Item[], focus: number, key: string, mods: { shift: boolean; alt: boolean }, range: ItemRange | null): ListResult {
  const stay: ListResult = { focus };
  const item = items[focus];
  if (item === undefined) return stay;
  const last = items.length - 1;
  const move = (to: number): ListResult => ({ focus: Math.max(0, Math.min(last, to)), range: null });

  switch (key) {
    case "ArrowUp":
    case "ArrowDown": {
      const down = key === "ArrowDown";
      if (mods.alt) {
        for (let i = focus + (down ? 1 : -1); i >= 0 && i <= last; i += down ? 1 : -1) if (items[i]!.kind === "file") return move(i);
        return stay;
      }
      if (!mods.shift) return move(focus + (down ? 1 : -1));
      const anchor = range?.from ?? focus;
      const end = range?.to ?? focus;
      if (!takesRange(items[anchor])) return stay;
      const next = stepRow(items, end, down ? 1 : -1);
      if (next === null || !takesRange(items[next]) || reach(items, anchor, next) !== next) return { focus: end };
      return { focus: next, range: next === anchor ? null : { from: anchor, to: next } };
    }
    case "ArrowLeft":
      if (item.kind === "file") return item.file.rows.length > 0 && !item.file.folded ? { focus, fold: item.file.path } : stay;
      return { focus: fileIndexOf(items, focus), range: null };
    case "ArrowRight":
      if (item.kind !== "file") return stay;
      return item.file.folded ? { focus, unfold: item.file.path } : move(focus + 1);
    case "Home":
      return move(0);
    case "End":
      return move(last);
    case "Enter":
      if (item.kind === "row" || item.kind === "comment") return { focus, open: "comment" };
      if (item.kind === "show") return { focus, unfold: item.file.path };
      if (item.kind === "file") {
        if (item.file.folded) return { focus, unfold: item.file.path };
        return item.file.rows.length > 0 ? { focus, fold: item.file.path } : stay;
      }
      return stay;
    case "F10":
    case "ContextMenu":
      return { focus, open: "menu" };
    case "Escape":
      return { focus, escape: true };
    default:
      return stay;
  }
}

const SIGN: Record<Row["kind"], string> = { hunk: "", context: " ", add: "+", del: "−" };
const LETTER: Record<Section["status"], string> = { modified: "M", added: "A", untracked: "A", deleted: "D", renamed: "R" };

function Chevron() {
  return (
    <svg className="changes__chevron" width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m5.5 4 3 3-3 3" />
    </svg>
  );
}

type ItemProps = {
  item: Item;
  index: number;
  tabStop: boolean;
  selected: boolean;
  onClick(index: number, e: ReactMouseEvent<HTMLElement>): void;
  onOpen(index: number): void;
  onMenu(index: number, at: { x: number; y: number }): void;
  /** The token classes of a row's text, when its section is coloured. */
  tokens: LineTokens | undefined;
};

/** A row's text in spans of the token classes; the text itself when nothing in it is coloured. */
function Coloured({ text, tokens }: { text: string; tokens: LineTokens | undefined }) {
  if (!tokens || tokens.length === 0) return <>{text}</>;
  const parts: ReactNode[] = [];
  let at = 0;
  for (const t of tokens) {
    if (t.from < at || t.to > text.length) continue;
    if (t.from > at) parts.push(text.slice(at, t.from));
    parts.push(
      <span key={t.from} className={t.cls}>
        {text.slice(t.from, t.to)}
      </span>,
    );
    at = t.to;
  }
  if (at < text.length) parts.push(text.slice(at));
  return <>{parts}</>;
}

function Entry({ item, index, tabStop, selected, onClick, onOpen, onMenu, tokens }: ItemProps) {
  const common = {
    role: "treeitem",
    "data-changes-item": "",
    "data-changes-index": index,
    "aria-level": item.kind === "file" ? 1 : 2,
    "aria-label": itemLabel(item),
    tabIndex: tabStop ? 0 : -1,
    onClick: (e: ReactMouseEvent<HTMLElement>) => onClick(index, e),
    onDoubleClick: () => onOpen(index),
    onContextMenu: (e: ReactMouseEvent<HTMLElement>) => {
      e.preventDefault();
      onMenu(index, { x: e.clientX, y: e.clientY });
    },
  };
  const mark = (name: string) => selectedProps(selected, "selected", `changes__item ${name}`);
  switch (item.kind) {
    case "file": {
      const { file } = item;
      return (
        <div {...common} {...mark("changes__file-head")} data-status={file.status} aria-expanded={!file.folded}>
          <span className="changes__twisty" aria-hidden="true">
            {file.rows.length > 0 && <Chevron />}
          </span>
          <span className="changes__path" title={file.oldPath === undefined ? file.path : `${file.oldPath} → ${file.path}`}>
            {file.path}
          </span>
          <span className="changes__status" aria-hidden="true">
            {LETTER[file.status]}
          </span>
          <span className="changes__counts" aria-hidden="true">
            <span>{`+${file.added}`}</span> <span>{`−${file.removed}`}</span>
          </span>
        </div>
      );
    }
    case "show":
      return (
        <div {...common} {...mark("changes__show")} data-kind="show">
          <span className="changes__note-text">{itemLabel(item)}</span>
        </div>
      );
    case "note":
      return (
        <div {...common} {...mark("changes__note")} data-kind="note">
          <span className="changes__note-text">{item.text}</span>
        </div>
      );
    case "comment": {
      const range = item.comment.start !== item.comment.end;
      return (
        <div {...common} {...mark("changes__comment")} data-kind="comment">
          <span className="changes__comment-text">{item.comment.text}</span>
          {item.placed.changed && <span className="changes__comment-changed">{copy.changed(range)}</span>}
        </div>
      );
    }
    case "row": {
      const row = item.file.rows[item.at]!;
      return (
        <div {...common} {...mark("changes__row")} data-kind={row.kind}>
          <span className="changes__no" aria-hidden="true">
            {row.kind === "hunk" ? "" : (row.newNo ?? row.oldNo)}
          </span>
          <span className="changes__sign" aria-hidden="true">
            {SIGN[row.kind]}
          </span>
          <span className="changes__code" aria-hidden="true">
            <Coloured text={row.text} tokens={tokens} />
          </span>
        </div>
      );
    }
    default: {
      const unreachable: never = item;
      return unreachable;
    }
  }
}

export type DiffListProps = {
  items: Item[];
  /** The index of the one tab stop, and of the item with the keyboard. */
  focus: number;
  range: ItemRange | null;
  wrap: boolean;
  /** A key the list takes, as `listKey` read it. */
  onKey(result: ListResult): void;
  /** A click or Shift+click; the parent decides what it moves, folds or widens. */
  onClickItem(index: number, mods: { shift: boolean }): void;
  /** A double-click: open the file (at the row's line). */
  onOpenItem(index: number): void;
  /** A right-click on an item, at the pointer. */
  onMenuItem(index: number, at: { x: number; y: number }): void;
  /** The token classes of each section's rows, by path. */
  colours?: Map<string, LineTokens[]>;
};

/** The whole diff as one tree: a block per file, its rows in blocks of 200, every item a `treeitem`. */
export function DiffList({ items, focus, range, wrap, onKey, onClickItem, onOpenItem, onMenuItem, colours }: DiffListProps) {
  const box = useRef<HTMLDivElement>(null);
  const keyed = useRef(0);

  // The item with the keyboard takes the focus after a key or a shift-click moved it (never because a poll changed the list).
  useEffect(() => {
    if (Date.now() - keyed.current >= KEY_FOCUS_MS) return;
    keyed.current = 0;
    box.current?.querySelector<HTMLElement>(`[data-changes-index="${focus}"]`)?.focus();
  }, [focus]);

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.defaultPrevented || e.ctrlKey || e.metaKey) return;
    const mods = { shift: e.shiftKey, alt: e.altKey };
    if (!isListKey(e.key, mods)) return;
    e.preventDefault();
    keyed.current = Date.now();
    onKey(listKey(items, focus, e.key, mods, range));
  };

  const lo = range === null ? -1 : Math.min(range.from, range.to);
  const hi = range === null ? -1 : Math.max(range.from, range.to);
  const entry = (i: number) => {
    const it = items[i]!;
    return (
      <Entry
        key={it.key}
        item={it}
        tokens={it.kind === "row" ? colours?.get(it.file.path)?.[it.at] : undefined}
        onMenu={onMenuItem}
        index={i}
        tabStop={i === focus}
        selected={i >= lo && i <= hi && it.kind === "row"}
        onClick={(index, e) => {
          if (e.shiftKey) keyed.current = Date.now();
          onClickItem(index, { shift: e.shiftKey });
        }}
        onOpen={onOpenItem}
      />
    );
  };

  // One group per file: from its header to the item before the next header.
  const files: number[][] = [];
  items.forEach((item, i) => {
    if (item.kind === "file" || files.length === 0) files.push([]);
    files[files.length - 1]!.push(i);
  });

  return (
    <div
      ref={box}
      className="changes__list"
      role="tree"
      aria-label="Changes"
      aria-multiselectable="true"
      data-wrap={wrap ? "true" : undefined}
      onKeyDown={onKeyDown}
      // A shift-press must not move the keyboard before the click reads where the range started.
      onMouseDown={(e) => e.shiftKey && e.preventDefault()}
    >
      {files.map((group) => {
        const chunks: number[][] = [];
        for (let a = 0; a < group.length; a += CHUNK) chunks.push(group.slice(a, a + CHUNK));
        return (
          <div key={items[group[0]!]!.key} className="changes__section" role="none">
            {chunks.map((chunk) => (
              <div key={chunk[0]} className="changes__chunk" role="none">
                {chunk.map(entry)}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}
