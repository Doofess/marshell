import type { SearchReply } from "../../generated/SearchReply";

export type FilterMode = { kind: "none" } | { kind: "names"; query: string } | { kind: "search"; query: string } | { kind: "short" };

export const SHORT_NOTE = "Type at least 2 characters to search.";

/** What the box's text asks for: names, "?" and the text to search, a "?" with too little text, or nothing. */
export function filterMode(text: string): FilterMode {
  const t = text.trim();
  if (t === "") return { kind: "none" };
  if (t.startsWith("?")) {
    const query = t.slice(1).trim();
    return query.length < 2 ? { kind: "short" } : { kind: "search", query };
  }
  return { kind: "names", query: t };
}

/** The line under the results: what stopped the search, that nothing matched, or that it failed. */
export function searchNote(reply: SearchReply | null, query: string, failed: boolean): string | null {
  if (failed) return "The search failed. Try again.";
  if (!reply) return null;
  if (reply.files.length === 0) return `No file has “${query}”.`;
  if (reply.cut === "lines") return "Showing the first 1,000 matching lines.";
  if (reply.cut === "time") return "The search stopped after 3 seconds. Showing what it found.";
  return null;
}

/** The line under the name filter's tree. */
export function namesNote(r: { matches: number; capped: boolean; listCut: boolean }, query: string): string | null {
  if (r.matches === 0) return `No file or folder name has “${query}”.`;
  if (r.capped) return "Showing 500 matches. Type more to narrow them.";
  if (r.listCut) return "Only the first 20,000 files are filtered.";
  return null;
}
