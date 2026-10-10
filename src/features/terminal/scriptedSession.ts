export const SCRIPTED_SIZE = { cols: 76, rows: 26 };

const ESC = "\x1b[";
const reset = `${ESC}0m`;
const sgr = (codes: string, text: string) => `${ESC}${codes}m${text}${reset}`;
const rgb = (r: number, g: number, b: number, text: string) => sgr(`38;2;${r};${g};${b}`, text);
const bg = (r: number, g: number, b: number, text: string) => sgr(`48;2;${r};${g};${b}`, text);
const orange = (t: string) => rgb(217, 119, 87, t);
/** Greys a program picks for a dark terminal: light ones vanish on white, dark ones on black. */
const hintGrey = (t: string) => rgb(170, 170, 170, t);
const deepGrey = (t: string) => rgb(48, 48, 48, t);
const dim = (t: string) => sgr("90", t);
const red = (t: string) => sgr("31", t);
const green = (t: string) => sgr("32", t);
const yellow = (t: string) => sgr("33", t);
const cyan = (t: string) => sgr("36", t);
const bold = (t: string) => sgr("1", t);

const BOX = 60;
const top = (title: string) => {
  const head = `\u256d\u2500\u2500 ${title} `;
  return orange(head + "\u2500".repeat(BOX - [...head].length - 1) + "\u256e");
};
const bottom = () => orange(`\u2570${"\u2500".repeat(BOX - 2)}\u256f`);
const boxRow = (plain: string, style: (t: string) => string) =>
  `${orange("\u2502")} ${style(plain)}${" ".repeat(BOX - 4 - [...plain].length)} ${orange("\u2502")}`;

/** A short Claude Code session as raw terminal output: palette colours, 24-bit colours and background fills. */
export const SCRIPTED_LINES: string[] = [
  top("Claude Code"),
  boxRow("Welcome back", bold),
  boxRow("Opus 5.5 \u00b7 high effort \u00b7 C:/dev/payments", hintGrey),
  bottom(),
  "",
  `${sgr("1;36", ">")} fix the flaky retry test in billing`,
  "",
  `${green("\u25cf")} I'll read the test and the retry helper first.`,
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 84 lines")}`,
  `${green("\u25cf")} ${bold("Read")}(src/billing/retry.test.ts)`,
  `  ${dim("\u23bf")}  ${hintGrey("Read 61 lines")}`,
  "",
  `${green("\u25cf")} The backoff uses the wall clock, so the test races it. Fixing the helper.`,
  "",
  `${green("\u25cf")} ${bold("Update")}(src/billing/retry.ts)`,
  `  ${dim("\u23bf")}  ${deepGrey("Updated with 2 additions and 1 removal")}`,
  `     ${bg(70, 30, 34, "- const wait = Date.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ const wait = clock.now() + base * 2 ** attempt;")}`,
  `     ${bg(28, 66, 40, "+ await clock.sleep(wait - clock.now());")}`,
  "",
  `${green("\u25cf")} ${bold("Bash")}(npm test)`,
  `  ${dim("\u23bf")}  ${green("PASS")} src/billing/retry.test.ts ${dim("(0.41s)")}`,
  `     ${yellow("1 warning")}: ${cyan("clock.now")} is deprecated in tests`,
  `     ${green("12 passed")}, ${red("0 failed")}`,
  `${dim("? for shortcuts")}`,
];

export const SCRIPTED_SESSION = `${SCRIPTED_LINES.join("\r\n")}\r\n`;
