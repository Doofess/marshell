import type { ITheme } from "@xterm/xterm";

/** Files longer than this (in characters) stay plain text with a note: highlighting a huge file would freeze the pane. */
export const HIGHLIGHT_CAP = 300 * 1024;

export type Token = { content: string; color?: string; bold?: boolean; italic?: boolean; underline?: boolean };

const PREFIX = "--code-";
const THEME_NAME = "marshell";

/**
 * The CSS custom properties the Shiki CSS-variables theme reads, set from the active terminal theme's ANSI colours, so code
 * in the preview follows the same theme as the terminal beside it. Comments are a mix of the text and the background, which
 * holds its contrast on any theme.
 */
export function themeVariables(t: ITheme): Record<string, string> {
  const fg = t.foreground ?? "CanvasText";
  const bg = t.background ?? "Canvas";
  const v = (name: string, value: string | undefined, fallback: string) => [`${PREFIX}${name}`, value ?? fallback] as const;
  return Object.fromEntries([
    v("foreground", fg, fg),
    v("background", bg, bg),
    v("token-keyword", t.magenta, fg),
    v("token-string", t.green, fg),
    v("token-string-expression", t.brightGreen, fg),
    v("token-constant", t.cyan, fg),
    v("token-function", t.blue, fg),
    v("token-parameter", t.brightYellow, fg),
    v("token-punctuation", fg, fg),
    v("token-link", t.brightBlue, fg),
    v("token-inserted", t.green, fg),
    v("token-deleted", t.red, fg),
    v("token-changed", t.yellow, fg),
    v("token-comment", `color-mix(in srgb, ${fg} 62%, ${bg})`, fg),
  ]);
}

// Shiki is loaded on first use, by dynamic import only, so none of it lands in the main chunk.
type Highlighter = {
  loadLanguage: (lang: unknown) => Promise<void>;
  getLoadedLanguages: () => string[];
  codeToTokensBase: (code: string, o: { lang: string; theme: string }) => Array<Array<{ content: string; color?: string; fontStyle?: number }>>;
};
let highlighter: Promise<Highlighter> | null = null;

function load(): Promise<Highlighter> {
  highlighter ??= (async () => {
    const [{ createHighlighterCore, createCssVariablesTheme }, { createJavaScriptRegexEngine }] = await Promise.all([import("shiki/core"), import("shiki/engine/javascript")]);
    const theme = createCssVariablesTheme({ name: THEME_NAME, variablePrefix: PREFIX, fontStyle: true });
    return (await createHighlighterCore({ themes: [theme], langs: [], engine: createJavaScriptRegexEngine() })) as unknown as Highlighter;
  })().catch((e: unknown) => {
    highlighter = null;
    throw e;
  });
  return highlighter;
}

const ITALIC = 1;
const BOLD = 2;
const UNDERLINE = 4;

/** The file's lines as coloured tokens, or null when Shiki or the language can't be loaded (the plain text then stays). */
export async function highlightLines(text: string, lang: string): Promise<Token[][] | null> {
  try {
    const { bundledLanguages } = (await import("shiki/langs")) as { bundledLanguages: Record<string, unknown> };
    const grammar = bundledLanguages[lang];
    if (!grammar) return null;
    const hl = await load();
    if (!hl.getLoadedLanguages().includes(lang)) await hl.loadLanguage(grammar);
    return hl.codeToTokensBase(text, { lang, theme: THEME_NAME }).map((line) =>
      line.map((t) => ({
        content: t.content,
        color: t.color,
        bold: !!((t.fontStyle ?? 0) & BOLD) || undefined,
        italic: !!((t.fontStyle ?? 0) & ITALIC) || undefined,
        underline: !!((t.fontStyle ?? 0) & UNDERLINE) || undefined,
      })),
    );
  } catch {
    return null;
  }
}
