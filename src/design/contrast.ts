import { differenceCiede2000, formatHex, interpolate, parse, wcagContrast } from "culori";
import type { ThemeValue } from "./cssTokens";

const ciede = differenceCiede2000();

function color(s: string) {
  const c = parse(s);
  if (!c) throw new Error(`Not a colour: ${s}`);
  return c;
}

/** WCAG 2 contrast ratio, 1 to 21. */
export function contrast(a: string, b: string): number {
  return wcagContrast(color(a), color(b));
}

/** CIEDE2000 colour difference; about 2 is barely visible, 15 or more is clearly different at a glance. */
export function deltaE(a: string, b: string): number {
  return ciede(color(a), color(b));
}

export const SURFACES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;

/** The lowest contrast of `c` against the three surfaces in one theme. */
export function minOnSurfaces(c: string, tokens: Record<string, ThemeValue>, theme: "light" | "dark"): number {
  return Math.min(...SURFACES.map((s) => contrast(c, tokens[s]![theme])));
}

/** `fg` at `alpha` over `bg`, mixed in oklch the way CSS color-mix does; returns a hex string. */
export function overlay(fg: string, bg: string, alpha: number): string {
  return formatHex(interpolate([color(bg), color(fg)], "oklch")(alpha));
}
