/** Files longer than this (in characters) stay plain text with a note: highlighting a huge file would freeze the pane. */
export const HIGHLIGHT_CAP = 300 * 1024;

export type Token = { content: string; color?: string; bold?: boolean; italic?: boolean; underline?: boolean };

const PREFIX = "--code-";
const THEME_NAME = "marshell";

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
