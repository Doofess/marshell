/** The plain-shell panel is as wide as the sidebar allows: a 288 px sidebar with 8 px of padding is 34 columns. */
export const SHELL_SIZE = { cols: 34, rows: 9 };

export type ShellName = "PowerShell" | "zsh" | "bash";

const ESC = "\x1b[";
const sgr = (codes: string, text: string) => `${ESC}${codes}m${text}${ESC}0m`;
const green = (t: string) => sgr("32", t);
const yellow = (t: string) => sgr("33", t);
const dim = (t: string) => sgr("90", t);

/** The longest folder name that still leaves the prompt and `git status -s` inside 34 columns. */
const MAX_NAME = 9;
const fit = (name: string) => ([...name].length > MAX_NAME ? `${[...name].slice(0, MAX_NAME - 1).join("")}…` : name);

const BACKSLASH = "\\";
const prompts: Record<ShellName, (name: string) => string> = {
  PowerShell: (name) => `PS C:${BACKSLASH}dev${BACKSLASH}${name}> `,
  zsh: (name) => `${name} % `,
  bash: (name) => `${name}$ `,
};

/**
 * A short plain-shell session as raw terminal output, for the sidebar's shell panel. It belongs to no agent: it is what
 * a person types when they just need a command prompt. It ends on a fresh prompt with no newline, so the cursor sits there.
 */
export function shellSession({ shell = "PowerShell", cwd = "my-app" }: { shell?: ShellName; cwd?: string } = {}): string {
  const prompt = prompts[shell](fit(cwd));
  return [`${prompt}git status -s`, ` ${yellow("M")} src/auth.ts`, `${prompt}npm test`, dim("> my-app@1.4.0 test"), `Tests: ${green("12 passed")}, 12 total`, prompt].join("\r\n");
}
