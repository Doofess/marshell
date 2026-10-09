import type { ITheme } from "@xterm/xterm";

// Phase 1: the terminal follows the app's light/dark theme. Phase 2 adds per-profile themes.
const dark: ITheme = { background: "#000000", foreground: "#e5e5e5", cursor: "#e5e5e5", selectionBackground: "#3a3a3a" };
const light: ITheme = { background: "#ffffff", foreground: "#1c1917", cursor: "#1c1917", selectionBackground: "#d6d3d1" };

export function currentTheme(): ITheme {
  return matchMedia("(prefers-color-scheme: dark)").matches ? dark : light;
}
