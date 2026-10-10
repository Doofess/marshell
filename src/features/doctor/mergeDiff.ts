type Hunk = { oldStart: number; oldLen: number; newLen: number; rest: string; body: string[] };

const HEAD = /^@@ -(\d+)(?:,(\d+))? \+\d+(?:,(\d+))? @@(.*)$/;

function split(diff: string): { header: string[]; hunks: Hunk[] } {
  const lines = diff.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  const header: string[] = [];
  const hunks: Hunk[] = [];
  for (const l of lines) {
    const h = HEAD.exec(l);
    if (h) hunks.push({ oldStart: Number(h[1]), oldLen: h[2] === undefined ? 1 : Number(h[2]), newLen: h[3] === undefined ? 1 : Number(h[3]), rest: h[4] ?? "", body: [] });
    else if (hunks.length === 0) header.push(l);
    else hunks[hunks.length - 1]!.body.push(l);
  }
  return { header, hunks };
}

/**
 * One diff for one file out of several diffs of that file, each made against the file as it was before any fix. It keeps the
 * first file header, orders the hunks by where they sit in the file, and moves each hunk's new-side start by what the earlier
 * hunks added or removed, so the result reads like the single diff of all the changes together. Phase 3 gets the combined
 * diff from the core's ChangeSet instead.
 */
export function mergeDiffs(diffs: string[]): string {
  const parts = diffs.filter((d) => d !== "");
  if (parts.length === 0) return "";
  if (parts.length === 1) return parts[0]!;
  const header = split(parts[0]!).header;
  const hunks = parts.flatMap((d) => split(d).hunks).sort((a, b) => a.oldStart - b.oldStart);
  // Two changes that touch the same lines cannot be one diff: they would not apply together. Keep each as its own section.
  if (hunks.some((h, i) => i > 0 && h.oldStart < hunks[i - 1]!.oldStart + hunks[i - 1]!.oldLen)) return parts.join("");
  let delta = 0;
  const out = [...header];
  for (const h of hunks) {
    const newStart = Math.max(h.oldLen === 0 ? 0 : 1, h.oldStart + delta);
    out.push(`@@ -${h.oldStart},${h.oldLen} +${newStart},${h.newLen} @@${h.rest}`, ...h.body);
    delta += h.newLen - h.oldLen;
  }
  return out.join("\n") + "\n";
}
