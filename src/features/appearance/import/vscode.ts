import { baseName, makeTheme, requireColor } from "./colors";
import { parseJsonc } from "./jsonc";
import type { ImportedTheme } from "./types";

const ANSI_NAMES = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];
const ANSI_KEYS = [...ANSI_NAMES.map((n) => `terminal.ansi${n}`), ...ANSI_NAMES.map((n) => `terminal.ansiBright${n}`)];

/** True when a parsed JSON file looks like a VS Code colour theme. */
export function looksLikeVsCode(data: unknown): boolean {
  return typeof data === "object" && data !== null && ("colors" in data || "tokenColors" in data);
}

/** Reads the terminal colours of a VS Code colour theme. `fileName` names a theme that has no `name`. */
export function parseVsCodeTheme(text: string, fileName = ""): ImportedTheme {
  const data = parseJsonc(text);
  const raw = typeof data === "object" && data !== null ? (data as { name?: unknown; colors?: unknown }) : {};
  const colors: Record<string, unknown> =
    typeof raw.colors === "object" && raw.colors !== null && !Array.isArray(raw.colors) ? (raw.colors as Record<string, unknown>) : {};
  if (!ANSI_KEYS.some((k) => k in colors)) throw new Error("This VS Code theme has no terminal colours.");

  const ansi = ANSI_KEYS.map((key) => requireColor(key, colors[key]));
  const pick = (...keys: string[]) => {
    const key = keys.find((k) => colors[k] !== undefined);
    return key === undefined ? undefined : requireColor(key, colors[key]);
  };
  const foreground = pick("terminal.foreground", "editor.foreground");
  const background = pick("terminal.background", "editor.background");
  if (!foreground) throw new Error('The theme is missing "terminal.foreground". Add it as #rrggbb and import again.');
  if (!background) throw new Error('The theme is missing "terminal.background". Add it as #rrggbb and import again.');

  const name = typeof raw.name === "string" && raw.name.trim() !== "" ? raw.name.trim() : baseName(fileName) || "VS Code theme";
  return makeTheme("vscode", name, {
    foreground,
    background,
    cursor: pick("terminalCursor.foreground") ?? foreground,
    selection: pick("terminal.selectionBackground") ?? "#808080",
    ansi,
  });
}

