import type { ToolDetail } from "./types";

export const COLLAPSED_LINES = 4;
/** Characters per line in a 12 px mono payload inside a 288 px-wide card, used to estimate wrapped lines. */
const DEFAULT_COLS = 44;
const SIGN = { context: "  ", add: "+ ", del: "- " } as const;

/** The payload as text lines (what the card shows, and what a screen reader reads). */
export function payloadLines(d: ToolDetail): string[] {
  switch (d.kind) {
    case "bash":
      return d.command.split("\n");
    case "edit":
      return d.hunk.map((l) => `${SIGN[l.kind]}${l.text}`);
    case "write":
      return d.preview.split("\n");
    case "fetch":
      return [d.url];
    case "mcp":
      return Object.entries(d.args).map(([k, v]) => `${k}: ${v}`);
    case "question":
      return [d.question];
  }
}

/** Lines after wrapping: every line takes at least one row, plus one per `cols` characters beyond the first. */
export function estimateLines(lines: string[], cols = DEFAULT_COLS): number {
  return lines.reduce((n, l) => n + Math.max(1, Math.ceil(l.length / cols)), 0);
}

export function isTruncated(lines: string[], cols = DEFAULT_COLS): boolean {
  return estimateLines(lines, cols) > COLLAPSED_LINES;
}

export function showAllLabel(total: number): string {
  return `Show all ${total} lines (Space)`;
}
