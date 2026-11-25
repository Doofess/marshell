export type Segment = { label: string; folder: string | null }; // folder: root-relative path to reveal; null for the file and for a path outside the project

const parts = (p: string) => p.split(/[\\/]+/).filter((s) => s !== "");

/** Whether `inner` starts with every part of `outer`, ignoring case on Windows. */
function under(outer: string[], inner: string[], windows: boolean): boolean {
  if (outer.length > inner.length) return false;
  const same = (a: string, b: string) => (windows ? a.toLowerCase() === b.toLowerCase() : a === b);
  return outer.every((s, i) => same(s, inner[i]!));
}

/**
 * The path bar's segments: the project's root, the folders and the file for a path in the project; `~` for one under home;
 * else the drive (or `/`) and on. Only a folder inside the project reveals itself.
 */
export function pathSegments(root: string | null, path: string | null, o: { windows: boolean; home: string | null }): Segment[] {
  if (root === null) return [];
  const rootParts = parts(root);
  const rootLabel = rootParts[rootParts.length - 1] ?? "/";
  if (path === null) return [{ label: rootLabel, folder: "" }];
  const p = parts(path);
  if (under(rootParts, p, o.windows)) {
    const rest = p.slice(rootParts.length);
    const out: Segment[] = [{ label: rootLabel, folder: "" }];
    rest.forEach((label, i) => out.push({ label, folder: i === rest.length - 1 ? null : rest.slice(0, i + 1).join("/") }));
    return out;
  }
  const homeParts = o.home === null ? [] : parts(o.home);
  if (homeParts.length > 0 && under(homeParts, p, o.windows)) {
    return [{ label: "~", folder: null }, ...p.slice(homeParts.length).map((label) => ({ label, folder: null }))];
  }
  if (o.windows) return p.map((label) => ({ label, folder: null }));
  return [{ label: "/", folder: null }, ...p.map((label) => ({ label, folder: null }))];
}

/**
 * Whether the path bar's path is scrolled to its end (a path that fits is). The scroll is negative in a right-to-left layout, and
 * a fraction of a pixel short of the end still counts.
 */
export function pathAtEnd(el: { scrollLeft: number; clientWidth: number; scrollWidth: number }): boolean {
  return Math.abs(el.scrollLeft) + el.clientWidth >= el.scrollWidth - 1;
}
