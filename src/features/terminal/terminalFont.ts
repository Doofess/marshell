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

/** The fonts shipped with the app (src/styles/fonts.css), in the order the picker lists them. */
export const BUNDLED_FONTS = ["JetBrains Mono", "Cascadia Mono", "Atkinson Hyperlegible Mono"] as const;
export type BundledFont = (typeof BUNDLED_FONTS)[number];

/** What the saved family string means: Match my terminal, one of the bundled fonts, or any other installed font. */
export type FontChoice = { kind: "match" } | { kind: "bundled"; family: BundledFont } | { kind: "other"; family: string };

export function fontChoiceOf(family: string | null | undefined): FontChoice {
  const text = family?.trim() ?? "";
  if (!text) return { kind: "match" };
  const bundled = BUNDLED_FONTS.find((f) => f.toLowerCase() === text.toLowerCase());
  return bundled ? { kind: "bundled", family: bundled } : { kind: "other", family: text };
}

/** The family string that is saved for a choice. */
export function familyOf(choice: FontChoice): string {
  return choice.kind === "match" ? "" : choice.family;
}

/**
 * The CSS font stack xterm gets: the family, the bundled font for any glyph it lacks, system monospace, then Windows
 * Terminal's (Nerd) font so powerline glyphs still resolve.
 */
export function fontStack(family: string, wtFamily?: string): string {
  const clean = (s: string) => s.replace(/["\\]/g, "");
  const name = clean(family);
  const stack = name === BUNDLED_FONT ? [`"${BUNDLED_FONT}"`, "monospace"] : [`"${name}"`, `"${BUNDLED_FONT}"`, "monospace"];
  const wt = wtFamily ? clean(wtFamily).trim() : "";
  if (wt && wt.toLowerCase() !== name.toLowerCase() && wt.toLowerCase() !== BUNDLED_FONT.toLowerCase()) stack.push(`"${wt}"`);
  return stack.join(", ");
}
