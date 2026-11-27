import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const src = new URL("../", import.meta.url);
const root = new URL("../../", import.meta.url);
const sources = (readdirSync(src, { recursive: true }) as string[])
  .map((f) => f.replaceAll("\\", "/"))
  .filter((f) => /\.(ts|tsx|css)$/.test(f) && !/\.test\.tsx?$/.test(f));

/**
 * Each way the picker chose an accent: the root's attribute, Storybook's global, the review page's select, and an inline
 * `--accent` written over the stylesheet's. Reading the token (`v["--accent"]`) is fine.
 */
const CHOOSES_ACCENT = /data-accent|dataset\.accent|globals\.accent|id="accent"|["']--accent(-ink)?["']\s*:|setProperty\(\s*["']--accent/;

describe("the accent choice", () => {
  it("the pattern catches each way the picker chose an accent", () => {
    for (const line of [
      `root.dataset.accent = "teal";`,
      `<html data-accent="teal">`,
      `const accent = ctx.globals.accent;`,
      `<select id="accent">`,
      `let style = { "--accent": check.hex, "--accent-ink": check.ink };`,
      `el.style.setProperty("--accent", hex);`,
    ])
      expect(line).toMatch(CHOOSES_ACCENT);
  });

  it("reads the source files it guards", () => {
    expect(sources).toContain("features/appearance/rootState.ts");
    expect(sources).toContain("styles/accents.css");
  });

  it("nothing sets or selects a chosen accent any more: sand is the only app accent", () => {
    const offenders = sources.filter((f) => CHOOSES_ACCENT.test(readFileSync(new URL(f, src), "utf8")));
    expect(offenders).toEqual([]);
    for (const f of [".storybook/preview.tsx", "scripts/build-review.mjs"]) {
      expect(readFileSync(new URL(f, root), "utf8"), f).not.toMatch(CHOOSES_ACCENT);
    }
  });
});
