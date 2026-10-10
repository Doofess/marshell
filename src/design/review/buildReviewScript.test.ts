import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const script = readFileSync(new URL("../../../scripts/build-review.mjs", import.meta.url), "utf8");

describe("scripts/build-review.mjs", () => {
  it("builds the latest batch when none is given, so a bare run never skips newer work", () => {
    expect(script).toMatch(/process\.argv\[3\] \?\? "2c"/);
  });
  it("says in its usage line that a batch can be given", () => {
    expect(script).toMatch(/usage: node scripts\/build-review\.mjs <out\.html> \[1\|2a\|2b\|2c\]/);
  });
});
