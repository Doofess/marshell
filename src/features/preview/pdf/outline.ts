import type { PDFDocumentProxy } from "pdfjs-dist";
import { placeOfDest } from "./dest";
import { goalOf, type LinkGoal } from "./links";

/** An entry of the PDF's outline. `id` is the index path ("0", "0.2"); `open` is a positive `count`. */
export type OutlineNode = { id: string; title: string; goal: LinkGoal | null; page: number | null; open: boolean; children: OutlineNode[] };

/** The outline keeps at most this many entries, in document order; the rest are dropped. */
export const MAX_ENTRIES = 10_000;
/** How many destinations are resolved at once. */
const RESOLVING = 8;

type Raw = { title?: unknown; dest?: unknown; url?: unknown; count?: unknown; items?: unknown };

/** A title as shown: control characters dropped, trimmed, and "Untitled" when nothing is left. */
function titleOf(raw: unknown): string {
  const t = typeof raw === "string" ? raw.replace(/\p{Cc}/gu, "").trim() : "";
  return t === "" ? "Untitled" : t;
}

function goalOfItem(item: Raw): LinkGoal | null {
  if (typeof item.url === "string") return goalOf({ url: item.url });
  if (typeof item.dest === "string" || Array.isArray(item.dest)) return { kind: "dest", dest: item.dest };
  return null;
}

/**
 * The outline of a document: entries named, capped and with the page each destination is on. A document that fails to give
 * one reads as having none. The walk keeps its own stack, so a deeply nested outline cannot overflow the call stack.
 */
export async function loadOutline(doc: PDFDocumentProxy): Promise<OutlineNode[]> {
  try {
    const raw = await doc.getOutline();
    if (!Array.isArray(raw)) return [];
    const out: OutlineNode[] = [];
    const withDest: OutlineNode[] = [];
    const stack: { items: readonly unknown[]; into: OutlineNode[]; prefix: string; at: number }[] = [{ items: raw, into: out, prefix: "", at: 0 }];
    let made = 0;
    while (stack.length > 0 && made < MAX_ENTRIES) {
      const frame = stack[stack.length - 1]!;
      if (frame.at >= frame.items.length) {
        stack.pop();
        continue;
      }
      const item = frame.items[frame.at++] as Raw | null;
      if (item === null || typeof item !== "object") continue;
      const node: OutlineNode = {
        id: `${frame.prefix}${frame.into.length}`,
        title: titleOf(item.title),
        goal: goalOfItem(item),
        page: null,
        open: typeof item.count === "number" && item.count > 0,
        children: [],
      };
      frame.into.push(node);
      made++;
      if (node.goal?.kind === "dest") withDest.push(node);
      if (Array.isArray(item.items) && item.items.length > 0) stack.push({ items: item.items, into: node.children, prefix: `${node.id}.`, at: 0 });
    }
    // The pages are found a few at a time, never all at once.
    let next = 0;
    const work = async () => {
      while (next < withDest.length) {
        const node = withDest[next++]!;
        const goal = node.goal as { kind: "dest"; dest: Parameters<typeof placeOfDest>[1] };
        node.page = (await placeOfDest(doc, goal.dest))?.page ?? null;
      }
    };
    await Promise.all(Array.from({ length: Math.min(RESOLVING, withDest.length) }, work));
    return out;
  } catch {
    return [];
  }
}

/** The entry whose section holds `page`: the last one in document order whose page is at most `page`, or null before the first. */
export function sectionAt(nodes: readonly OutlineNode[], page: number): string | null {
  let found: string | null = null;
  const stack: { items: readonly OutlineNode[]; at: number }[] = [{ items: nodes, at: 0 }];
  while (stack.length > 0) {
    const frame = stack[stack.length - 1]!;
    if (frame.at >= frame.items.length) {
      stack.pop();
      continue;
    }
    const node = frame.items[frame.at++]!;
    if (node.page !== null && node.page <= page) found = node.id;
    if (node.children.length > 0) stack.push({ items: node.children, at: 0 });
  }
  return found;
}

/** Every entry by its id. */
export function indexNodes(nodes: readonly OutlineNode[]): Map<string, OutlineNode> {
  const map = new Map<string, OutlineNode>();
  const stack: { items: readonly OutlineNode[]; at: number }[] = [{ items: nodes, at: 0 }];
  while (stack.length > 0) {
    const frame = stack[stack.length - 1]!;
    if (frame.at >= frame.items.length) {
      stack.pop();
      continue;
    }
    const node = frame.items[frame.at++]!;
    map.set(node.id, node);
    if (node.children.length > 0) stack.push({ items: node.children, at: 0 });
  }
  return map;
}

/** A row of the tree as it is shown: `level` counts from 1, `posinset` from 1 among its siblings. */
export type Row = { id: string; level: number; title: string; parent: string | null; hasChildren: boolean; expanded: boolean; posinset: number; setsize: number };

/** The rows on screen: every entry whose parents are all in `expanded`, in document order. */
export function visibleRows(nodes: readonly OutlineNode[], expanded: ReadonlySet<string>): Row[] {
  const rows: Row[] = [];
  const stack: { items: readonly OutlineNode[]; at: number; level: number; parent: string | null }[] = [{ items: nodes, at: 0, level: 1, parent: null }];
  while (stack.length > 0) {
    const frame = stack[stack.length - 1]!;
    if (frame.at >= frame.items.length) {
      stack.pop();
      continue;
    }
    const index = frame.at++;
    const node = frame.items[index]!;
    const hasChildren = node.children.length > 0;
    const isOpen = hasChildren && expanded.has(node.id);
    rows.push({ id: node.id, level: frame.level, title: node.title, parent: frame.parent, hasChildren, expanded: isOpen, posinset: index + 1, setsize: frame.items.length });
    if (isOpen) stack.push({ items: node.children, at: 0, level: frame.level + 1, parent: node.id });
  }
  return rows;
}

/** The ids above an entry, outermost first: "0.2.1" is under "0.2", which is under "0". */
export function ancestorsOf(id: string): string[] {
  const parts = id.split(".");
  return parts.slice(0, -1).map((_, i) => parts.slice(0, i + 1).join("."));
}

type TreeMove = { focus: string } | { expand: string } | { collapse: string } | { activate: string };

/** What a key does in the tree, from the row that has the keyboard: Up, Down, Home and End move, Right and Left open, close and climb, Enter jumps. */
export function treeKey(rows: readonly Row[], at: string, key: string): TreeMove | null {
  const i = rows.findIndex((r) => r.id === at);
  const row = rows[i];
  if (!row) return null;
  const go = (to: number): TreeMove | null => (to === i || !rows[to] ? null : { focus: rows[to]!.id });
  switch (key) {
    case "ArrowDown":
      return go(i + 1);
    case "ArrowUp":
      return go(i - 1);
    case "Home":
      return go(0);
    case "End":
      return go(rows.length - 1);
    case "ArrowRight":
      if (!row.hasChildren) return null;
      return row.expanded ? go(i + 1) : { expand: row.id };
    case "ArrowLeft":
      if (row.hasChildren && row.expanded) return { collapse: row.id };
      return row.parent === null ? null : { focus: row.parent };
    case "Enter":
      return { activate: row.id };
    default:
      return null;
  }
}
