import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PALETTES } from "../terminal/palettes";
import { HIGHLIGHT_CAP, highlightLines, themeVariables } from "./highlight";

describe("themeVariables", () => {
  const theme = PALETTES.dark;
  const vars = themeVariables(theme);
  it("maps the code colours to the terminal theme's ANSI colours", () => {
    expect(vars["--code-token-keyword"]).toBe(theme.magenta);
    expect(vars["--code-token-string"]).toBe(theme.green);
    expect(vars["--code-token-function"]).toBe(theme.blue);
    expect(vars["--code-token-constant"]).toBe(theme.cyan);
    expect(vars["--code-foreground"]).toBe(theme.foreground);
  });
  it("sets every variable the CSS-variables theme reads", () => {
    for (const k of ["foreground", "background", "token-constant", "token-string", "token-comment", "token-keyword", "token-parameter", "token-function", "token-string-expression", "token-punctuation", "token-link", "token-inserted", "token-deleted", "token-changed"])
      expect(vars[`--code-${k}`], k).toBeTruthy();
  });
});

describe("highlightLines", () => {
  it("returns one token list per line that joins back to the text, coloured through the code variables", async () => {
    const text = "const a = 1;\nreturn a;\n";
    const lines = await highlightLines(text, "typescript");
    expect(lines).not.toBeNull();
    expect(lines!.map((l) => l.map((t) => t.content).join("")).join("\n")).toBe(text);
    expect(lines!.flat().some((t) => t.color?.includes("--code-token-keyword"))).toBe(true);
  });
  it("is null for a language Shiki doesn't have, so the plain text stays", async () => {
    expect(await highlightLines("x", "no-such-language")).toBeNull();
  });
  it("only highlights up to 300 KB", () => {
    expect(HIGHLIGHT_CAP).toBe(300 * 1024);
  });
});

describe("how Shiki is loaded", () => {
  const walk = (dir: string): string[] =>
    readdirSync(dir).flatMap((f) => {
      const p = join(dir, f);
      return statSync(p).isDirectory() ? walk(p) : /\.tsx?$/.test(f) ? [p] : [];
    });
  it("is imported dynamically only, so it never lands in the main chunk", () => {
    const offenders = walk(join(__dirname, "..", "..")).filter((p) => /^\s*(import|export)[^;]*from\s+["']shiki/m.test(readFileSync(p, "utf8")) && !/\.test\.tsx?$/.test(p));
    expect(offenders).toEqual([]);
  });
});
