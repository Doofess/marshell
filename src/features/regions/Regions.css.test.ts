import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { rules } from "../../components/a11y/cssRules";

const css = rules(readFileSync(new URL("./Regions.css", import.meta.url), "utf8"));
const forced = (selector: string) => css.filter((r) => r.forced && r.selectors.includes(selector));

describe("the region box in forced colours", () => {
  // The dimming box-shadow is dropped there, so the box's own edge is all that marks it, also once focus has moved to the comment.
  it("has a permanent dashed outline in a system colour, whether or not it is focused", () => {
    const always = forced(".region-box");
    expect(always.length).toBeGreaterThan(0);
    expect(always.map((r) => r.body).join(";")).toMatch(/outline:\s*\d+px dashed (CanvasText|Highlight)/);
  });
  it("keeps a different shape for focus (a double line), so focus can still be told from rest", () => {
    expect(forced(".region-box:focus").map((r) => r.body).join(";")).toMatch(/outline:\s*\d+px double Highlight/);
  });
});
