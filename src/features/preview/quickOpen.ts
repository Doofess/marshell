import { matchRanges, score, type Range } from "../launcher/filter";

/** A match on the file name itself counts for more than the same letters scattered through the folders. */
const NAME_BONUS = 10;

/** The file name and the folder it is in, from a `/`-separated path. */
export function splitPath(path: string): { name: string; dir: string } {
  const i = path.lastIndexOf("/");
  return i < 0 ? { name: path, dir: "" } : { name: path.slice(i + 1), dir: path.slice(0, i) };
}

const slashed = (p: string) => p.replace(/\\/g, "/");

/**
 * The path as the palette shows and matches it: relative to the session's root when it is inside it, otherwise absolute.
 * Always `/`-separated; the root is compared without regard to case (Windows) and only at a folder boundary.
 */
export function displayPath(abs: string, root: string): string {
  const a = slashed(abs);
  const r = slashed(root).replace(/\/+$/, "");
  return r !== "" && a.length > r.length && a.slice(0, r.length).toLowerCase() === r.toLowerCase() && a[r.length] === "/" ? a.slice(r.length + 1) : a;
}

/** The path to open for a listed (root-relative) path, with the separator the root uses. */
export function openPath(rel: string, root: string): string {
  const sep = root.includes("\\") ? "\\" : "/";
  return root.replace(/[\\/]+$/, "") + sep + (sep === "\\" ? rel.replace(/\//g, "\\") : rel);
}

/** A display path that is not relative to the root: it starts with a separator or a drive. */
const isOutside = (path: string) => /^([a-z]:)?\//i.test(path);

/** The part of a display path that is matched and scored: for a path outside the root, its folder and file name only. */
function scoredPart(path: string): { text: string; offset: number } {
  if (!isOutside(path)) return { text: path, offset: 0 };
  const text = path.split("/").slice(-2).join("/");
  return { text, offset: path.length - text.length };
}

export type FileHit ={ path: string; ranges: Range[] };

/**
 * Fuzzy-ranks display paths (see `displayPath`) against a query: best score first, a match on the file name with a bonus, and
 * a path outside the root scored on its folder and file name only, a path from `seen` ahead of an equal score from `files`. An empty query returns the `seen` paths in the order given.
 */
export function rankFiles(query: string, files: string[], seen: string[], limit: number): FileHit[] {
  if (query.trim() === "") return seen.slice(0, limit).map((path) => ({ path, ranges: [] }));
  const scored: Array<FileHit & { score: number; seen: boolean }> = [];
  const add = (path: string, isSeen: boolean) => {
    const { text, offset } = scoredPart(path);
    const local = matchRanges(query, text);
    if (local === null) return;
    const name = splitPath(path).name;
    const inName = matchRanges(query, name);
    // Outside the root, the letters marked are the file name's when it matches, not a scatter across the folder.
    const from = offset > 0 && inName !== null ? { ranges: inName, at: path.length - name.length } : { ranges: local, at: offset };
    const ranges = from.ranges.map(([a, b]): Range => [a + from.at, b + from.at]);
    scored.push({ path, ranges, score: score(local, text) + (inName === null ? 0 : NAME_BONUS + score(inName, name)), seen: isSeen });
  };
  for (const p of seen) add(p, true);
  for (const p of files) add(p, false);
  return scored
    .sort((a, b) => b.score - a.score || Number(b.seen) - Number(a.seen))
    .slice(0, limit)
    .map(({ path, ranges }) => ({ path, ranges }));
}
