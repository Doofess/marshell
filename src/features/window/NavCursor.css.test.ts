import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./NavCursor.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const outsideForced = css.replace(/@media \(forced-colors: active\) \{(?:[^{}]|\{[^{}]*\})*\}/g, "");

describe("NavCursor.css", () => {
  it("tells the target apart by a fill and a start-edge bar, never a box, a ring or an outline", () => {
    expect(outsideForced).not.toMatch(/(?:^|[\s;{])border(?:-[a-z]+)*:\s*(?!0\b)[\d.]+px\s+solid/m);
    expect(outsideForced).not.toMatch(/outline\s*:/);
    expect(outsideForced).not.toMatch(/box-shadow:\s*(?:inset\s+)?0\s+0\s+0\s+[\d.]+px/);
    expect(outsideForced).toMatch(/\.nav-cursor\s*\{[^}]*background:\s*color-mix\(in oklch, var\(--accent\)/);
    expect(outsideForced).toMatch(/\.nav-cursor::before\s*\{[^}]*inline-size:\s*var\(--stripe\)/);
  });
  it("is made of tokens: no hex, rgb or raw colour", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b/i);
    expect(css).not.toMatch(/\brgba?\(/);
    expect(css).not.toMatch(/(?<![\w-])oklch\(/);
  });
  it("moves only when motion is welcome, and animates named properties, never all", () => {
    const motion = css.match(/@media \(prefers-reduced-motion: no-preference\) \{(?:[^{}]|\{[^{}]*\})*\}/)?.[0] ?? "";
    expect(motion).toMatch(/transition-property:\s*inset-inline-start/);
    expect(css.replace(motion, "")).not.toMatch(/transition/);
    expect(css).not.toMatch(/transition(?:-property)?:\s*all/);
  });
  it("keeps the cursor visible in Windows High Contrast with a system-colour outline and bar", () => {
    const forced = css.match(/@media \(forced-colors: active\) \{(?:[^{}]|\{[^{}]*\})*\}/)?.[0] ?? "";
    expect(forced).toMatch(/\.nav-cursor\s*\{[^}]*outline:\s*2px solid Highlight/);
    expect(forced).toMatch(/\.nav-cursor::before\s*\{[^}]*background:\s*Highlight/);
    expect(forced).toMatch(/\.nav-hint\s*\{[^}]*outline:\s*1px solid CanvasText/);
  });
  it("never scrolls and never catches the pointer", () => {
    expect(css).not.toMatch(/overflow(?:-[xy])?:\s*(?:auto|scroll)/);
    expect(css).toMatch(/\.nav-cursor\s*\{[^}]*pointer-events:\s*none/);
    expect(css).toMatch(/\.nav-hint\s*\{[^}]*pointer-events:\s*none/);
  });
  it("uses logical properties only", () => {
    expect(css).not.toMatch(/(?:^|[\s;{])(?:left|right|top|bottom|margin-left|margin-right|padding-left|padding-right|width|height)\s*:/m);
  });
});
