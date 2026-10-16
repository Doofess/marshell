import { contrast } from "../../design/contrast";

/**
 * The three surfaces of each theme, as in src/styles/tokens.css (customAccent.test.ts keeps the two in step).
 * Copied rather than read because the page cannot parse its own stylesheet source.
 */
export const ACCENT_SURFACES = {
  light: ["oklch(100% 0 none)", "oklch(98.2% 0.003 85)", "oklch(95.6% 0.004 75)"],
  dark: ["oklch(0% 0 none)", "oklch(14.5% 0 none)", "oklch(19.1% 0 none)"],
} as const;

/** `#abc`, `abc`, `#AABBCC` or `aabbcc` as `#aabbcc`; null for anything else. */
export function normalizeHex(input: string): string | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(input.trim());
  if (!m) return null;
  const h = m[1]!.toLowerCase();
  return `#${h.length === 3 ? [...h].map((c) => c + c).join("") : h}`;
}

const ratio = (n: number) => `${(Math.floor(n * 10) / 10).toFixed(1)}:1`;

/**
 * A custom accent is used in both themes for fills and focus rings, so it needs 3:1 on every surface of each (WCAG 1.4.11),
 * and black or white text on it needs 4.5:1.
 */
export function checkAccent(input: string): { ok: true; hex: string; ink: "#000000" | "#ffffff" } | { ok: false; reason: string } {
  const hex = normalizeHex(input);
  if (!hex) return { ok: false, reason: "Enter a colour as #rrggbb, for example #3a7bd5." };
  const light = Math.min(...ACCENT_SURFACES.light.map((s) => contrast(hex, s)));
  if (light < 3) return { ok: false, reason: `Too light for the light theme: ${ratio(light)} against the background, needs 3:1.` };
  const dark = Math.min(...ACCENT_SURFACES.dark.map((s) => contrast(hex, s)));
  if (dark < 3) return { ok: false, reason: `Too dark for the dark theme: ${ratio(dark)} against the background, needs 3:1.` };
  const onBlack = contrast(hex, "#000000");
  const onWhite = contrast(hex, "#ffffff");
  const best = Math.max(onBlack, onWhite);
  if (best < 4.5) return { ok: false, reason: `Text on this colour would be hard to read: ${ratio(best)}, needs 4.5:1.` };
  return { ok: true, hex, ink: onBlack >= onWhite ? "#000000" : "#ffffff" };
}
