export type DiffSection = { path: string; diff: string };

const HEADER = "diff --git ";

/**
 * The path in the text after `diff --git `. For a file that was not renamed that is `a/X b/X`, so the text is cut in half at
 * the " b/" that makes both sides equal: right even when X holds a space or " b/". A rename (`a/OLD b/NEW`) is told apart by its
 * own lines, so the new name is read from `rename to`, then `+++ b/`, and last from the final " b/".
 */
function pathOf(rest: string, lines: string[]): string {
  const n = (rest.length - 5) / 2;
  if (Number.isInteger(n) && n > 0 && rest.startsWith("a/") && rest.slice(2 + n, 5 + n) === " b/" && rest.slice(2, 2 + n) === rest.slice(5 + n)) return rest.slice(5 + n);
  const renamed = lines.find((l) => l.startsWith("rename to "));
  if (renamed) return renamed.slice("rename to ".length);
  const plus = lines.find((l) => l.startsWith("+++ b/"));
  if (plus) return plus.slice("+++ b/".length);
  const i = rest.lastIndexOf(" b/");
  return i < 0 ? rest : rest.slice(i + 3);
}

/**
 * One entry per file of a git diff, in order, each holding that file's lines whole (so it can be handed to a diff view as it is).
 * A line inside a hunk always starts with a space, `+`, `-` or `\`, so a line that starts with `diff --git ` is always a new file.
 */
export function splitDiff(diff: string): DiffSection[] {
  const starts = [...diff.matchAll(/^diff --git /gm)].map((m) => m.index);
  return starts.map((from, i) => {
    const section = diff.slice(from, starts[i + 1] ?? diff.length);
    const lines = section.split("\n").map((l) => l.replace(/\r$/, ""));
    return { path: pathOf(lines[0]!.slice(HEADER.length), lines), diff: section };
  });
}

/**
 * "binary" or "large" for a section the core only names (it has a `Binary file P` or `Large file P` line in its header and no
 * hunks), else null. Only the header is read, so a changed line that happens to say "Binary file" is not mistaken for one.
 */
export function sectionNote(section: string): "binary" | "large" | null {
  for (const l of section.split("\n").slice(1)) {
    if (l.startsWith("@@") || l.startsWith("--- ")) return null;
    if (l.startsWith("Binary file")) return "binary";
    if (l.startsWith("Large file ")) return "large";
  }
  return null;
}
