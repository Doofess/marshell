import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/**
 * Every global stylesheet in `src/styles/` reaches the app through `main.tsx`. Storybook imports its own set, so a sheet
 * left out of the entry still looks right in stories while the app never loads it (motion.css was missing this way, and no
 * row flash, glyph bounce or vendor loop ever ran in the app).
 */
const main = readFileSync(new URL("../main.tsx", import.meta.url), "utf8");
const sheets = readdirSync(new URL(".", import.meta.url)).filter((name) => name.endsWith(".css"));

describe("the app entry", () => {
  it.each(sheets)("imports %s", (sheet) => {
    expect(main).toContain(`import "./styles/${sheet}";`);
  });
});
