import type { OutputBuffer } from "../preview/outputPaths";

/** The output buffer of each live terminal, by tab id, for what reads a session's output from outside its view (quick open's "Seen in output"). */
const buffers = new Map<string, () => OutputBuffer>();

/** Registers a terminal's buffer; the returned function removes it, unless a newer terminal for the tab has taken the place. */
export function registerTerminal(tab: string, buffer: () => OutputBuffer): () => void {
  buffers.set(tab, buffer);
  return () => {
    if (buffers.get(tab) === buffer) buffers.delete(tab);
  };
}

export const terminalBuffer = (tab: string): OutputBuffer | null => buffers.get(tab)?.() ?? null;
