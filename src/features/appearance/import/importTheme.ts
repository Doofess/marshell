import { parseJsonc } from "./jsonc";
import { parseItermColors } from "./iterm";
import type { ImportedTheme } from "./types";
import { looksLikeVsCode, parseVsCodeTheme } from "./vscode";
import { looksLikeWt, parseWtSchemes } from "./windowsTerminal";

export const MAX_THEME_FILE = 1_000_000;
const TOO_BIG = "This file is too big to be a theme (over 1 MB).";
const UNKNOWN = "Marshell can read Windows Terminal .json, iTerm2 .itermcolors and VS Code theme .json files.";

export type ImportResult = { ok: true; themes: ImportedTheme[] } | { ok: false; reason: string };

/** Picks a parser by extension and content. Never throws: a problem with the file comes back as a reason to show. */
export function importThemeFile(name: string, text: string): ImportResult {
  if (text.length > MAX_THEME_FILE) return { ok: false, reason: TOO_BIG };
  try {
    const lower = name.toLowerCase();
    if (lower.endsWith(".itermcolors")) return { ok: true, themes: [parseItermColors(text, name)] };
    if (lower.endsWith(".json")) {
      const data = parseJsonc(text);
      if (looksLikeWt(data)) return { ok: true, themes: parseWtSchemes(text) };
      if (looksLikeVsCode(data)) return { ok: true, themes: [parseVsCodeTheme(text, name)] };
    }
    return { ok: false, reason: UNKNOWN };
  } catch (e) {
    return { ok: false, reason: e instanceof Error ? e.message : String(e) };
  }
}
