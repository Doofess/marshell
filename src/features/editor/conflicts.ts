/** The 1-based line numbers of a conflict's marker lines. `base` is the `|||||||` line when there is one. */
export type Conflict = { start: number; base: number | null; middle: number; end: number };

const START = /^<{7}( .*)?$/;
const BASE = /^\|{7}( .*)?$/;
const MIDDLE = "=======";
const END = /^>{7}( .*)?$/;

/**
 * The conflicts in the lines of a text: a start line, optionally a base line, a middle line and an end line, in
 * that order. A new start line before the end starts the conflict over; a base or middle line outside an open
 * conflict is ignored; an end line before a middle line drops the open conflict.
 */
export function conflictsIn(lines: Iterable<string>): Conflict[] {
  const found: Conflict[] = [];
  let start = 0;
  let base: number | null = null;
  let middle = 0;
  let n = 0;
  for (const line of lines) {
    n += 1;
    if (START.test(line)) {
      start = n;
      base = null;
      middle = 0;
    } else if (start === 0) {
      continue;
    } else if (middle === 0 && base === null && BASE.test(line)) {
      base = n;
    } else if (middle === 0 && line === MIDDLE) {
      middle = n;
    } else if (END.test(line)) {
      if (middle !== 0) found.push({ start, base, middle, end: n });
      start = 0;
      base = null;
      middle = 0;
    }
  }
  return found;
}

/** How many conflicts the text has. The text is split on `\n` only. */
export function countConflicts(text: string): number {
  return text === "" ? 0 : conflictsIn(text.split("\n")).length;
}

/** The index of the conflict to go to from `line`, or null when there are none. Both directions wrap around. */
export function conflictFrom(
  conflicts: readonly Conflict[],
  line: number,
  which: "first" | "next" | "previous",
): number | null {
  if (conflicts.length === 0) return null;
  if (which === "first") return 0;
  if (which === "next") {
    const i = conflicts.findIndex((c) => c.start > line);
    return i === -1 ? 0 : i;
  }
  for (let i = conflicts.length - 1; i >= 0; i -= 1) {
    if (conflicts[i]!.start < line) return i;
  }
  return conflicts.length - 1;
}
