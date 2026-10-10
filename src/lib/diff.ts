/** A git-style unified diff of two texts, or "" when they are equal. Line-level; phase 3 replaces it with the core's ChangeSet diff (same format). */
export function unifiedDiff(path: string, before: string, after: string, context = 3): string {
  if (before === after) return "";
  const a = before.split("\n");
  const b = after.split("\n");
  if (a[a.length - 1] === "") a.pop();
  if (b[b.length - 1] === "") b.pop();
  // Longest common subsequence table.
  const n = a.length;
  const m = b.length;
  const t: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) t[i]![j] = a[i] === b[j] ? t[i + 1]![j + 1]! + 1 : Math.max(t[i + 1]![j]!, t[i]![j + 1]!);
  type Op = { k: " " | "-" | "+"; s: string };
  const ops: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      ops.push({ k: " ", s: a[i++]! });
      j++;
    } else if (t[i + 1]![j]! >= t[i]![j + 1]!) ops.push({ k: "-", s: a[i++]! });
    else ops.push({ k: "+", s: b[j++]! });
  }
  while (i < n) ops.push({ k: "-", s: a[i++]! });
  while (j < m) ops.push({ k: "+", s: b[j++]! });

  const changed = ops.map((o, idx) => (o.k === " " ? -1 : idx)).filter((x) => x >= 0);
  const out = [`--- a/${path}`, `+++ b/${path}`];
  let k = 0;
  while (k < changed.length) {
    const start = Math.max(0, changed[k]! - context);
    let end = Math.min(ops.length, changed[k]! + context + 1);
    while (k + 1 < changed.length && changed[k + 1]! - context <= end) {
      k++;
      end = Math.min(ops.length, changed[k]! + context + 1);
    }
    k++;
    const slice = ops.slice(start, end);
    let oldStart = 1;
    let newStart = 1;
    for (const o of ops.slice(0, start)) {
      if (o.k !== "+") oldStart++;
      if (o.k !== "-") newStart++;
    }
    const oldLen = slice.filter((o) => o.k !== "+").length;
    const newLen = slice.filter((o) => o.k !== "-").length;
    out.push(`@@ -${oldStart},${oldLen} +${newStart},${newLen} @@`);
    for (const o of slice) out.push(o.k + o.s);
  }
  return out.join("\n") + "\n";
}
