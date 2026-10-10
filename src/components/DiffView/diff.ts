export type DiffKind = "meta" | "hunk" | "context" | "add" | "del";
export type DiffLine = { kind: DiffKind; text: string; oldNo?: number; newNo?: number };
export type ParsedDiff = { lines: DiffLine[]; added: number; removed: number; files: string[] };

export const SIGN: Record<DiffKind, string> = { meta: "", hunk: "", context: " ", add: "+", del: "−" };

/** Reads a git-style unified diff. Header lines are only headers outside a hunk, so a removed line that starts with dashes still counts as a removal. */
export function parseUnifiedDiff(input: string): ParsedDiff {
  const lines: DiffLine[] = [];
  const files: string[] = [];
  let added = 0;
  let removed = 0;
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  // What the hunk header promised: once both sides are used up the hunk is over, so the next "--- " starts the next file.
  let oldLeft = 0;
  let newLeft = 0;
  const raw = input.replace(/\r\n/g, "\n").split("\n");
  if (raw[raw.length - 1] === "") raw.pop();
  for (let i = 0; i < raw.length; i++) {
    const l = raw[i]!;
    if (inHunk && oldLeft <= 0 && newLeft <= 0) inHunk = false;
    // A file header that follows a hunk early (wrong counts) is still recognised: "---", "+++", then a hunk header.
    if (inHunk && l.startsWith("--- ") && raw[i + 1]?.startsWith("+++ ") && raw[i + 2]?.startsWith("@@")) inHunk = false;
    if (l.startsWith("diff ")) {
      inHunk = false;
      lines.push({ kind: "meta", text: l });
      continue;
    }
    if (!inHunk && (l.startsWith("--- ") || l.startsWith("+++ ") || l.startsWith("index "))) {
      if (l.startsWith("+++ ")) files.push(l.slice(4).replace(/^b\//, ""));
      lines.push({ kind: "meta", text: l });
      continue;
    }
    const h = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(l);
    if (h) {
      inHunk = true;
      oldNo = Number(h[1]);
      newNo = Number(h[3]);
      oldLeft = h[2] === undefined ? 1 : Number(h[2]);
      newLeft = h[4] === undefined ? 1 : Number(h[4]);
      lines.push({ kind: "hunk", text: l });
      continue;
    }
    if (l.startsWith("+")) {
      added++;
      newLeft--;
      lines.push({ kind: "add", text: l.slice(1), newNo: newNo++ });
    } else if (l.startsWith("-")) {
      removed++;
      oldLeft--;
      lines.push({ kind: "del", text: l.slice(1), oldNo: oldNo++ });
    } else {
      oldLeft--;
      newLeft--;
      lines.push({ kind: "context", text: l.startsWith(" ") ? l.slice(1) : l, oldNo: oldNo++, newNo: newNo++ });
    }
  }
  return { lines, added, removed, files };
}

export function collapse(lines: DiffLine[], limit: number): { shown: DiffLine[]; hidden: number } {
  return lines.length <= limit ? { shown: lines, hidden: 0 } : { shown: lines.slice(0, limit), hidden: lines.length - limit };
}
