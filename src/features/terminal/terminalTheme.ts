import type { ITheme } from "@xterm/xterm";
import { MIN_CONTRAST, PALETTES, type Scheme } from "./palettes";

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

export function terminalOptions(scheme: Scheme): { theme: ITheme; minimumContrastRatio: number } {
  return { theme: PALETTES[scheme], minimumContrastRatio: MIN_CONTRAST };
}
