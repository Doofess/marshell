import { nameParts, type TreeRow } from "./treeModel";

const CAP = 500;

/** The tree's order for names, as the core sorts a folder: digits as numbers, case ignored. */
const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" }).compare;

type Node = { path: string; name: string; folder: boolean; children: Node[]; open: boolean };

/**
 * The tree for a name filter, from git's file list. A file that matches is kept inside its
 * folders, which are open. A folder that matches shows closed unless `expanded` has it; then
 * every file under it is listed.
 */
export function filterRows(
  files: readonly string[],
  query: string,
  expanded: ReadonlySet<string>,
): { rows: TreeRow[]; matches: number; capped: boolean } {
  const q = query.toLowerCase();
  if (q === "") return { rows: [], matches: 0, capped: false };
  const hits = (name: string) => name.toLowerCase().includes(q);

  const root: Node = { path: "", name: "", folder: true, children: [], open: true };
  const byPath = new Map<string, Node>();
  const counted = new Set<string>();
  let matches = 0;
  let capped = false;

  const ensure = (parent: Node, name: string, folder: boolean, open: boolean): Node => {
    const path = parent.path === "" ? name : `${parent.path}/${name}`;
    let node = byPath.get(path);
    if (!node) {
      node = { path, name, folder, children: [], open };
      byPath.set(path, node);
      parent.children.push(node);
    }
    return node;
  };
  const count = (path: string): boolean => {
    if (counted.has(path)) return true;
    if (matches >= CAP) {
      capped = true;
      return false;
    }
    counted.add(path);
    matches++;
    return true;
  };

  for (const file of files) {
    const segs = file.split("/");
    const fileName = segs[segs.length - 1] ?? file;
    const fileHit = hits(fileName);
    // Walk down to find where the chain stops and whether the file is wanted.
    let wanted = fileHit;
    for (let i = 0; i < segs.length - 1 && !wanted; i++) wanted = hits(segs[i] ?? "");
    if (!wanted) continue;

    let parent = root;
    let prefix = "";
    let stopped = false;
    for (let i = 0; i < segs.length - 1; i++) {
      prefix = prefix === "" ? (segs[i] ?? "") : `${prefix}/${segs[i] ?? ""}`;
      const folderHit = hits(segs[i] ?? "");
      const open = folderHit ? expanded.has(prefix) : true;
      if (folderHit && !count(prefix)) {
        stopped = true;
        break;
      }
      parent = ensure(parent, segs[i] ?? "", true, open);
      if (!open) {
        stopped = true;
        break;
      }
    }
    if (capped) break;
    if (stopped) continue;
    if (fileHit && !count(file)) break;
    if (!fileHit) {
      // Listed only because a folder above matched and is open.
      if (matches >= CAP) {
        capped = true;
        break;
      }
      counted.add(file);
      matches++;
    }
    ensure(parent, fileName, false, false);
  }

  const rows: TreeRow[] = [];
  const walk = (node: Node, level: number) => {
    const sorted = [...node.children].sort((x, y) => Number(y.folder) - Number(x.folder) || byName(x.name, y.name));
    sorted.forEach((child, i) => {
      rows.push({
        path: child.path,
        name: child.name,
        kind: child.folder ? "folder" : "file",
        ignored: false,
        level,
        expanded: child.folder && child.open,
        loading: false,
        setSize: sorted.length,
        posInSet: i + 1,
        parts: nameParts(child.name, query),
      });
      if (child.folder && child.open) walk(child, level + 1);
    });
  };
  walk(root, 0);
  return { rows, matches, capped };
}
