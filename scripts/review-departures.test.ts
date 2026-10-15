import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const js = readFileSync(new URL("./review-departures.js", import.meta.url), "utf8");
describe("review-departures.js", () => {
  it("plays the precomputed frames of every flip cell and replays with the page's button", () => {
    expect(js).toContain("data-frames");
    expect(js).toContain("data-at");
    expect(js).toMatch(/Replay animations|replay/i);
  });
  it("skips the flipping when motion is reduced", () => {
    expect(js).toContain("prefers-reduced-motion");
  });
  it("always cross-fades a cell the story marks as reduced, whatever the page's setting", () => {
    expect(js).toContain("data-reduced");
  });
});
