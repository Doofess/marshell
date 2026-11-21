import type { TreeEntry } from "../../generated/TreeEntry";

export type Listing = {
  entries: TreeEntry[];
  more: number;
  stamp: number;
  state: "listed" | "gone" | "refused";
};
export type NamePart = { text: string; hit: boolean };
export type TreeRow = {
  path: string;
  name: string;
  kind: "file" | "folder" | "link" | "more" | "empty";
  ignored: boolean;
  level: number;
  expanded: boolean;
  loading: boolean;
  setSize: number;
  posInSet: number;
  more?: number;
  parts?: NamePart[];
};

/** Paths inside the tree are relative and use "/". */
export function childPath(dir: string, name: string): string {
  return dir === "" ? name : `${dir}/${name}`;
}

export function ancestors(path: string): string[] {
  const parts = path.split("/");
  const out: string[] = [];
  for (let i = 1; i < parts.length; i++) out.push(parts.slice(0, i).join("/"));
  return out;
}

function trimEnd(p: string, windows: boolean): string {
  return p.replace(windows ? /[\\/]+$/ : /\/+$/, "");
}

export function relativeTo(root: string, abs: string, windows: boolean): string | null {
  const norm = (p: string) => (windows ? p.replace(/\\/g, "/") : p);
  const r = trimEnd(norm(root), false);
  const a = norm(abs);
  const fold = (s: string) => (windows ? s.toLowerCase() : s);
  if (fold(a) === fold(r) || fold(a) === fold(r) + "/") return "";
  const prefix = fold(r) + "/";
  if (!fold(a).startsWith(prefix)) return null;
  return a.slice(prefix.length).replace(/\/+$/, "");
}

export function absoluteOf(root: string, rel: string, windows: boolean): string {
  if (rel === "") return root;
  const sep = windows ? "\\" : "/";
  const base = trimEnd(root, windows);
  const tail = windows ? rel.replace(/\//g, "\\") : rel;
  return `${base}${sep}${tail}`;
}

export function openFolders(rows: readonly TreeRow[]): string[] {
  return rows.filter((r) => r.kind === "folder" && r.expanded).map((r) => r.path);
}

export function visibleRows(listings: ReadonlyMap<string, Listing>, expanded: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (dir: string, level: number) => {
    const listing = listings.get(dir);
    if (!listing || listing.state !== "listed") return;
    const size = listing.entries.length;
    listing.entries.forEach((entry, i) => {
      const path = childPath(dir, entry.name);
      const isFolder = entry.kind === "folder";
      const open = isFolder && expanded.has(path);
      rows.push({
        path,
        name: entry.name,
        kind: entry.kind,
        ignored: entry.ignored,
        level,
        expanded: open,
        loading: open && !listings.has(path),
        setSize: size,
        posInSet: i + 1,
      });
      if (open) {
        const child = listings.get(path);
        if (child && child.state === "listed" && child.entries.length === 0 && child.more === 0) {
          rows.push({
            path: childPath(path, "\u0000empty"),
            name: "",
            kind: "empty",
            ignored: false,
            level: level + 1,
            expanded: false,
            loading: false,
            setSize: 1,
            posInSet: 1,
          });
        } else {
          walk(path, level + 1);
        }
      }
    });
    if (listing.more > 0) {
      rows.push({
        path: childPath(dir, "\u0000more"),
        name: "",
        kind: "more",
        ignored: false,
        level,
        expanded: false,
        loading: false,
        setSize: 1,
        posInSet: 1,
        more: listing.more,
      });
    }
  };
  walk("", 0);
  return rows;
}

export type TreeMove =
  | { type: "focus"; index: number }
  | { type: "expand" | "collapse"; path: string }
  | { type: "open"; path: string; keep: boolean }
  | { type: "filter" }
  | { type: "menu" }
  | null;

export function treeKey(
  e: { key: string; ctrlKey: boolean; altKey: boolean; metaKey: boolean; shiftKey: boolean },
  rows: readonly TreeRow[],
  index: number,
  rtl: boolean,
): TreeMove {
  if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey)) {
    return { type: "menu" };
  }
  if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "f") return { type: "filter" };
  if (e.ctrlKey || e.altKey || e.metaKey) return null;
  const row = rows[index];
  const focus = (i: number): TreeMove => (i >= 0 && i < rows.length && i !== index ? { type: "focus", index: i } : null);
  const toggle = (r: TreeRow): TreeMove =>
    r.kind === "folder" ? { type: r.expanded ? "collapse" : "expand", path: r.path } : null;
  const isReal = (r: TreeRow | undefined): r is TreeRow => !!r && r.kind !== "more" && r.kind !== "empty";

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
      if (!isReal(row)) return null;
      const forward = (e.key === "ArrowRight") !== rtl;
      if (forward) {
        if (row.kind !== "folder") return null;
        if (!row.expanded) return { type: "expand", path: row.path };
        const next = rows[index + 1];
        return next && next.level === row.level + 1 && isReal(next) ? { type: "focus", index: index + 1 } : null;
      }
      if (row.kind === "folder" && row.expanded) return { type: "collapse", path: row.path };
      for (let i = index - 1; i >= 0; i--) {
        if ((rows[i]?.level ?? row.level) < row.level) return { type: "focus", index: i };
      }
      return null;
    }
    case "Enter":
      if (!isReal(row)) return null;
      return row.kind === "file" ? { type: "open", path: row.path, keep: true } : toggle(row);
    case " ":
      if (!isReal(row)) return null;
      return row.kind === "file" ? { type: "open", path: row.path, keep: false } : toggle(row);
    default:
      return null;
  }
}

/** The next row whose name starts with the typed letters, wrapping. A longer string starts at the current row. */
export function typeAhead(rows: readonly TreeRow[], from: number, typed: string): number | null {
  if (typed === "" || rows.length === 0) return null;
  const q = typed.toLowerCase();
  const start = typed.length > 1 ? from : from + 1;
  for (let n = 0; n < rows.length; n++) {
    const i = (((start + n) % rows.length) + rows.length) % rows.length;
    const r = rows[i];
    if (r && r.kind !== "more" && r.kind !== "empty" && r.name.toLowerCase().startsWith(q)) return i;
  }
  return null;
}

export function nameParts(name: string, query: string): NamePart[] {
  if (query === "") return [{ text: name, hit: false }];
  const lower = name.toLowerCase();
  const q = query.toLowerCase();
  const parts: NamePart[] = [];
  let pos = 0;
  for (;;) {
    const at = lower.indexOf(q, pos);
    if (at < 0) break;
    if (at > pos) parts.push({ text: name.slice(pos, at), hit: false });
    parts.push({ text: name.slice(at, at + q.length), hit: true });
    pos = at + q.length;
  }
  if (pos < name.length) parts.push({ text: name.slice(pos), hit: false });
  return parts.length > 0 ? parts : [{ text: name, hit: false }];
}
