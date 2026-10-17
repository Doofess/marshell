import { logicalLine, type BufferLike } from "../terminal/pathLinks";
import { findPathCandidates } from "./pathCandidates";
import { resolvePaths } from "./previewApi";

/** The buffer `outputPaths` reads: xterm's active buffer fits it. */
export type OutputBuffer = BufferLike & { readonly length: number };

const MAX_LINES = 2000;
/** The core refuses more candidates than this per resolve. */
const MAX_CANDIDATES = 64;

/** The distinct path-like texts in the last 2 000 lines of output, newest first, at most 64 (one resolve call's worth). */
export function outputPaths(buffer: OutputBuffer): string[] {
  const found = new Set<string>();
  for (let y = buffer.length - 1; y >= Math.max(0, buffer.length - MAX_LINES) && found.size < MAX_CANDIDATES; y--) {
    // A row that continues the one above is read with it, from the row it starts on.
    if (buffer.getLine(y)?.isWrapped) continue;
    const { text } = logicalLine(buffer, y);
    for (const c of findPathCandidates(text)) {
      if (found.size < MAX_CANDIDATES) found.add(c.text);
    }
  }
  return [...found];
}

type Resolver = (tab: string, candidates: string[]) => Promise<Array<string | null>>;

/** The files named in a session's recent output that exist: absolute paths, newest first, once each. */
export async function seenPaths(tab: string, buffer: OutputBuffer, resolve: Resolver = resolvePaths): Promise<string[]> {
  const candidates = outputPaths(buffer);
  if (candidates.length === 0) return [];
  const resolved = await resolve(tab, candidates);
  return [...new Set(resolved.filter((p): p is string => p !== null))];
}
