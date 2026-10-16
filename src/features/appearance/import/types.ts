// Local stand-in until src/generated/ImportedTheme.ts exists; switch the imports over to the generated type then.

export type TerminalColors = {
  foreground: string;
  background: string;
  cursor: string;
  selection: string;
  /** The 16 ANSI colours, black first, bright white last. */
  ansi: string[];
};

export type ImportedTheme = {
  id: string;
  name: string;
  /** "windows-terminal" | "iterm2" | "vscode" */
  source: string;
  colors: TerminalColors;
};
