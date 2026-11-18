import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { HIGHLIGHT_CAP, highlightLines, tokenizer } from "./highlight";

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

describe("tokenizer", () => {
  it("tokenizer_of_an_unknown_language_is_null", async () => {
    expect(await tokenizer("no-such-language")).toBeNull();
  });
  it("tokenizer_carries_a_block_comment_across_calls", async () => {
    const tok = (await tokenizer("typescript"))!;
    expect(tok).not.toBeNull();
    const first = tok(["/* a comment", "still inside"], undefined);
    const next = tok(["more comment */ const a = 1;"], first.state);
    const line = "more comment */ const a = 1;";
    const comment = next.tokens[0]!.filter((t) => t.cls.split(" ").includes("tok-comment"));
    expect(comment.length).toBeGreaterThan(0);
    expect(comment[0]!.from).toBe(0);
    // the comment ends with its closing mark, so "const" is not part of it
    expect(comment.at(-1)!.to).toBeLessThanOrEqual(line.indexOf("*/") + 2);
    const fresh = tok(["more comment */ const a = 1;"], undefined);
    expect(fresh.tokens[0]![0]!.cls.split(" ")).not.toContain("tok-comment");
    // a line that is wholly inside the comment is all tok-comment
    expect(first.tokens[1]).toEqual([{ from: 0, to: "still inside".length, cls: "tok-comment" }]);
    const inside = tok(["just words"], first.state);
    expect(inside.tokens[0]).toEqual([{ from: 0, to: "just words".length, cls: "tok-comment" }]);
  });
  it("tokenizer_skips_a_long_line", async () => {
    const tok = (await tokenizer("typescript"))!;
    const open = tok(["/* open"], undefined);
    const long = "x".repeat(2001);
    const r = tok(["const a = 1;", long, "still in comment?"], open.state);
    expect(r.tokens).toHaveLength(3);
    expect(r.tokens[1]).toEqual([]);
    const skipped = tok([long], open.state);
    expect(skipped.tokens).toEqual([[]]);
    expect(skipped.state).toBe(open.state);
  });
  it("tokenizer_names_classes_not_colours", async () => {
    const tok = (await tokenizer("typescript"))!;
    const line = "const a = 1; // note";
    const { tokens } = tok([line], undefined);
    const all = tokens[0]!;
    expect(all.length).toBeGreaterThan(0);
    for (const t of all) {
      expect(t.cls).toMatch(/^tok-[a-z0-9-]+( tok-[ibu])*$/);
      expect(t.cls).not.toContain("var(");
      expect(t.to).toBeGreaterThan(t.from);
    }
    const kw = all.find((t) => line.slice(t.from, t.to) === "const");
    expect(kw?.cls).toContain("tok-keyword");
    // the plain `a` and the spaces have the default colour, so they are left out
    expect(all.some((t) => line.slice(t.from, t.to) === " ")).toBe(false);
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
