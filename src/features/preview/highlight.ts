import { LONG_LINE } from "../editor/tokens";

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
  codeToTokens: (code: string, o: { lang: string; theme: string; grammarState?: unknown }) => { tokens: Array<Array<{ content: string; color?: string; fontStyle?: number }>>; grammarState?: unknown };
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

/** The tokens of one line: offsets in the line (UTF-16 units) and the token's classes. */
export type LineTokens = Array<{ from: number; to: number; cls: string }>;

/** Colours lines that follow the ones the state came from. The first call takes `undefined` as its state; the state it returns goes to the next call. */
export type Tokenizer = (lines: string[], state: unknown) => { tokens: LineTokens[]; state: unknown };

const DEFAULT_COLOUR = `var(${PREFIX}foreground)`;
const COLOUR = new RegExp(`^var\\(${PREFIX}(?:token-)?([a-z0-9-]+)\\)$`);

function classes(color: string | undefined, fontStyle: number): string {
  const out: string[] = [];
  if (color && color !== DEFAULT_COLOUR) {
    const m = COLOUR.exec(color);
    if (m) out.push(`tok-${m[1]}`);
  }
  if (fontStyle & ITALIC) out.push("tok-i");
  if (fontStyle & BOLD) out.push("tok-b");
  if (fontStyle & UNDERLINE) out.push("tok-u");
  return out.join(" ");
}

/** A tokenizer for the language, or null when Shiki doesn't have it or it fails to load. */
export async function tokenizer(lang: string): Promise<Tokenizer | null> {
  try {
    const { bundledLanguages } = (await import("shiki/langs")) as { bundledLanguages: Record<string, unknown> };
    const grammar = bundledLanguages[lang];
    if (!grammar) return null;
    const hl = await load();
    if (!hl.getLoadedLanguages().includes(lang)) await hl.loadLanguage(grammar);
    return (lines, state) => {
      const tokens: LineTokens[] = [];
      let run: string[] = [];
      const flush = () => {
        if (run.length === 0) return;
        const r = hl.codeToTokens(run.join("\n"), { lang, theme: THEME_NAME, grammarState: state ?? undefined });
        for (const line of r.tokens) {
          const out: LineTokens = [];
          let at = 0;
          for (const t of line) {
            const cls = classes(t.color, t.fontStyle ?? 0);
            if (cls) out.push({ from: at, to: at + t.content.length, cls });
            at += t.content.length;
          }
          tokens.push(out);
        }
        if (r.grammarState) state = r.grammarState;
        run = [];
      };
      for (const line of lines) {
        if (line.length > LONG_LINE) {
          flush();
          tokens.push([]);
        } else run.push(line);
      }
      flush();
      return { tokens, state };
    };
  } catch {
    return null;
  }
}
