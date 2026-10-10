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
  const raw = input.replace(/\r\n/g, "\n").split("\n");
  if (raw[raw.length - 1] === "") raw.pop();
  for (const l of raw) {
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
    const h = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(l);
    if (h) {
      inHunk = true;
      oldNo = Number(h[1]);
      newNo = Number(h[2]);
      lines.push({ kind: "hunk", text: l });
      continue;
    }
    if (l.startsWith("+")) {
      added++;
      lines.push({ kind: "add", text: l.slice(1), newNo: newNo++ });
    } else if (l.startsWith("-")) {
      removed++;
      lines.push({ kind: "del", text: l.slice(1), oldNo: oldNo++ });
    } else {
      lines.push({ kind: "context", text: l.startsWith(" ") ? l.slice(1) : l, oldNo: oldNo++, newNo: newNo++ });
    }
  }
  return { lines, added, removed, files };
}

export function collapse(lines: DiffLine[], limit: number): { shown: DiffLine[]; hidden: number } {
  return lines.length <= limit ? { shown: lines, hidden: 0 } : { shown: lines.slice(0, limit), hidden: lines.length - limit };
}
