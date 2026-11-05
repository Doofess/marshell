import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./LiveDoctor.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("LiveDoctor.css", () => {
  // Found in the 7a live check: with an `auto` row the row grew to a long diff, the sheet's `max-block-size: 100%`
  // had nothing to resolve against, and the sheet's buttons ended up below the window.
  it("sizes the sheet's row to the overlay, so a long fix keeps its buttons in the window", () => {
    expect(css).toMatch(/\.live-doctor__sheet\s*\{[^}]*grid-template-rows:\s*minmax\(0,\s*1fr\)/);
  });
});
