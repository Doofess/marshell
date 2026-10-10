import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./DiffView.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("DiffView.css", () => {
  it("tints added and removed lines from the status tokens, with no border", () => {
    expect(css).toMatch(/\.diff__line\[data-kind="add"\]\s*\{[^}]*color-mix\(in oklch, var\(--ok\)/);
    expect(css).toMatch(/\.diff__line\[data-kind="del"\]\s*\{[^}]*color-mix\(in oklch, var\(--error\)/);
    expect(css).not.toMatch(/border(?!-radius)/);
  });
  it("wraps long lines instead of overflowing or scrolling sideways", () => {
    expect(css).toMatch(/white-space:\s*pre-wrap/);
    expect(css).toMatch(/overflow-wrap:\s*anywhere/);
    expect(css).not.toMatch(/overflow(-x)?:\s*(auto|scroll)/);
  });
  it("uses the mono font and keeps the sign column fixed", () => {
    expect(css).toMatch(/font-family:\s*var\(--font-mono\)/);
    expect(css).toMatch(/\.diff__sign\s*\{[^}]*inline-size/);
  });
  it("keeps the hunk header readable on its stronger fill (AA): full-strength text, not the muted one", () => {
    expect(css).toMatch(/\.diff__line\[data-kind="hunk"\]\s*\{[^}]*color:\s*var\(--text-1\)/);
  });
});
