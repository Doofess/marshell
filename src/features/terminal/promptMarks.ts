/** One OSC 133 shell-integration mark: A prompt start, B command start, C command executed, D command finished. */
export type Osc133 = { kind: "A" | "B" | "C" | "D"; exit: number | null };

/** Reads the text after "133;" ("A", "D;1", "D;0;aid=2"). Null for a mark we don't know. Only D carries an exit code. */
export function parseOsc133(data: string): Osc133 | null {
  const [kind, code] = data.split(";");
  if (kind !== "A" && kind !== "B" && kind !== "C" && kind !== "D") return null;
  const exit = kind === "D" && code !== undefined && /^-?\d+$/.test(code) ? Number(code) : null;
  return { kind, exit };
}

/** The buffer line to scroll to, or null when there is no prompt in that direction. */
export function promptTarget(promptLines: number[], viewportTop: number, by: -1 | 1): number | null {
  if (by === -1) {
    const above = promptLines.filter((line) => line < viewportTop);
    return above.length === 0 ? null : Math.max(...above);
  }
  const below = promptLines.filter((line) => line > viewportTop);
  return below.length === 0 ? null : Math.min(...below);
}
