import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const src = new URL("../", import.meta.url);
const root = new URL("../../", import.meta.url);
const sources = (readdirSync(src, { recursive: true }) as string[]).filter((f) => /\.(ts|tsx|css)$/.test(f) && !/\.test\.tsx?$/.test(f));

describe("the accent choice", () => {
  it("nothing sets or selects a chosen accent any more: sand is the only app accent", () => {
    const offenders = sources.filter((f) => /data-accent|dataset\.accent/.test(readFileSync(new URL(f.replaceAll("\\", "/"), src), "utf8")));
    expect(offenders).toEqual([]);
    for (const f of [".storybook/preview.tsx", "scripts/build-review.mjs"]) {
      expect(readFileSync(new URL(f, root), "utf8"), f).not.toMatch(/data-accent|dataset\.accent|globals\.accent|id="accent"/);
    }
  });
});
