import type { ITheme } from "@xterm/xterm";
import type { ImportedTheme } from "../../generated/ImportedTheme";
import type { TerminalColors } from "../../generated/TerminalColors";
import { HIGH_CONTRAST_PALETTES, MIN_CONTRAST, PALETTES, type Scheme } from "./palettes";

export type { Scheme };
/** docs/PLAN.md "Terminal theme": follow the app by default, or pin the terminal dark or light. */
export type TerminalThemeSetting = "follow-app" | "dark" | "light";

export function resolveScheme(setting: TerminalThemeSetting, app: Scheme): Scheme {
  return setting === "follow-app" ? app : setting;
}

/** The scheme an element renders in, from its computed `color-scheme` and the OS preference as the fallback. */
export function schemeFromComputed(colorScheme: string, prefersDark: boolean): Scheme {
  if (colorScheme === "dark") return "dark";
  if (colorScheme === "light") return "light";
  return prefersDark ? "dark" : "light";
}

/** The scheme the app is showing right now (browser only). */
export function appScheme(root: HTMLElement = document.documentElement): Scheme {
  return schemeFromComputed(getComputedStyle(root).colorScheme, matchMedia("(prefers-color-scheme: dark)").matches);
}

/** Whether High contrast is on, from the root element's `data-contrast` (browser only). */
export function appHighContrast(root: HTMLElement = document.documentElement): boolean {
  return root.getAttribute("data-contrast") === "more";
}

export function terminalOptions(scheme: Scheme, highContrast = false): { theme: ITheme; minimumContrastRatio: number } {
  return { theme: (highContrast ? HIGH_CONTRAST_PALETTES : PALETTES)[scheme], minimumContrastRatio: MIN_CONTRAST };
}

const ANSI_NAMES = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
] as const;

/** An imported scheme as an xterm theme. The scrollbar stays hidden, as in the built-in palettes. */
export function themeFromColors(c: TerminalColors): ITheme {
  const theme: ITheme = {
    background: c.background,
    foreground: c.foreground,
    cursor: c.cursor,
    cursorAccent: c.background,
    selectionBackground: c.selection,
    scrollbarSliderBackground: "#00000000",
    scrollbarSliderHoverBackground: "#00000000",
    scrollbarSliderActiveBackground: "#00000000",
  };
  ANSI_NAMES.forEach((name, i) => {
    if (c.ansi[i]) theme[name] = c.ansi[i];
  });
  return theme;
}

const channel = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);

/** Dark when white text reads better on the background than black text does. */
export function schemeOfBackground(hex: string): Scheme {
  const n = parseInt(hex.slice(1, 7), 16);
  const lum = 0.2126 * channel((n >> 16) / 255) + 0.7152 * channel(((n >> 8) & 255) / 255) + 0.0722 * channel((n & 255) / 255);
  // Contrast with white beats contrast with black below this luminance.
  return lum < 0.179 ? "dark" : "light";
}

export type TerminalThemeInput = {
  /** The Appearance setting: "follow-app", "dark", "light" or an imported theme id. */
  setting: string;
  /** The profile's own terminal theme, which wins when it names one that exists. */
  override?: string | null;
  imported: Pick<ImportedTheme, "id" | "colors">[];
  app: Scheme;
  /** High contrast is on: the built-in palettes take the window's true black or pure white. Imported themes are unchanged. */
  highContrast?: boolean;
  protection: boolean;
};

/** The xterm theme for one terminal, and the scheme it reports to programs (COLORFGBG, mode 2031). */
export function resolveTerminalTheme(input: TerminalThemeInput): { theme: ITheme; scheme: Scheme; minimumContrastRatio: number } {
  const ratio = input.protection ? MIN_CONTRAST : 1;
  const pick = (name: string | null | undefined) => {
    if (name === "follow-app" || name === "dark" || name === "light") {
      const scheme = resolveScheme(name, input.app);
      return { theme: (input.highContrast ? HIGH_CONTRAST_PALETTES : PALETTES)[scheme], scheme };
    }
    const found = input.imported.find((t) => t.id === name);
    return found ? { theme: themeFromColors(found.colors), scheme: schemeOfBackground(found.colors.background) } : null;
  };
  const chosen = pick(input.override) ?? pick(input.setting) ?? pick("follow-app")!;
  return { ...chosen, minimumContrastRatio: ratio };
}
