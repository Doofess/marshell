import type { ITheme } from "@xterm/xterm";

export type Scheme = "dark" | "light";
/** Passed to xterm's minimumContrastRatio so colours a program painted for the other scheme stay readable. */
export const MIN_CONTRAST = 4.5;

/**
 * The default themes follow the window: each background is the base surface (--bg-base in tokens.css) as sRGB bytes, so no seam
 * shows between the terminal and the window. palettes.test.ts ties them to the CSS.
 */
export const PALETTES: Record<Scheme, ITheme> = {
  dark: {
    background: "#121212",
    foreground: "#e5e5e5",
    cursor: "#e5e5e5",
    cursorAccent: "#121212",
    selectionBackground: "#3a3a3a",
    scrollbarSliderBackground: "#00000000",
    scrollbarSliderHoverBackground: "#00000000",
    scrollbarSliderActiveBackground: "#00000000",
    black: "#3a3a3a",
    red: "#ff6b6b",
    green: "#5ad07a",
    yellow: "#e5c14a",
    blue: "#6ea8ff",
    magenta: "#d28bff",
    cyan: "#4fd1d1",
    white: "#c9c9c9",
    brightBlack: "#8a8a8a",
    brightRed: "#ff8a8a",
    brightGreen: "#7ee89a",
    brightYellow: "#ffd866",
    brightBlue: "#8fbaff",
    brightMagenta: "#e0a8ff",
    brightCyan: "#7de3e3",
    brightWhite: "#ffffff",
  },
  light: {
    background: "#f6f5f3",
    foreground: "#1c1917",
    cursor: "#1c1917",
    cursorAccent: "#f6f5f3",
    selectionBackground: "#d6d3d1",
    scrollbarSliderBackground: "#00000000",
    scrollbarSliderHoverBackground: "#00000000",
    scrollbarSliderActiveBackground: "#00000000",
    black: "#1c1917",
    red: "#c62828",
    green: "#1b7a3a",
    yellow: "#8a5a00",
    blue: "#1f5fbf",
    magenta: "#8e3bb8",
    cyan: "#0e7a85",
    white: "#6b6b6b",
    brightBlack: "#6e6e6e",
    brightRed: "#d32f2f",
    brightGreen: "#2e7d32",
    brightYellow: "#946200",
    brightBlue: "#296dd4",
    brightMagenta: "#9d4bc8",
    brightCyan: "#077c87",
    brightWhite: "#4a4a4a",
  },
};

/** The same palettes under High contrast, where the window is true black or pure white (appearance.css). */
export const HIGH_CONTRAST_PALETTES: Record<Scheme, ITheme> = {
  dark: { ...PALETTES.dark, background: "#000000", cursorAccent: "#000000" },
  light: { ...PALETTES.light, background: "#ffffff", cursorAccent: "#ffffff" },
};
