import type { TerminalFont } from "../../generated/TerminalFont";

/** Shipped with the app (src/styles/fonts.css), so the terminal always has a font with every box-drawing glyph. */
export const BUNDLED_FONT = "JetBrains Mono";

export type ResolvedFont = { family: string; size: number; source: "yours" | "windows-terminal" | "bundled" };

/**
 * The terminal's font: the one you set, else Windows Terminal's default profile font, else the bundled one.
 * The size is always yours; Windows Terminal's is only shown as a hint.
 */
export function resolveTerminalFont(setting: TerminalFont, wt: { family?: string; size?: number } | null): ResolvedFont {
  const yours = setting.family?.trim();
  if (yours) return { family: yours, size: setting.size, source: "yours" };
  const theirs = wt?.family?.trim();
  if (theirs) return { family: theirs, size: setting.size, source: "windows-terminal" };
  return { family: BUNDLED_FONT, size: setting.size, source: "bundled" };
}

/** The CSS font stack xterm gets: the family, then the bundled font for any glyph it lacks. */
export function fontStack(family: string): string {
  const name = family.replace(/["\\]/g, "");
  return name === BUNDLED_FONT ? `"${BUNDLED_FONT}", monospace` : `"${name}", "${BUNDLED_FONT}", monospace`;
}
