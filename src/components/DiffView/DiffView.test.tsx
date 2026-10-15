import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { DiffView } from "./DiffView";

const small = ["--- a/s.json", "+++ b/s.json", "@@ -1,1 +1,2 @@", " a", "-b", "+c", "+d"].join("\n");
const big = ["--- a/s.json", "+++ b/s.json", "@@ -1,1 +1,200 @@", ...Array.from({ length: 200 }, (_, i) => `+line ${i}`)].join("\n");

describe("DiffView", () => {
  it("marks every changed line with a sign as well as a fill, and counts them", () => {
    const h = renderToStaticMarkup(<DiffView diff={small} label="Changes to s.json" />);
    expect(h).toContain('data-kind="add"');
    expect(h).toContain('data-kind="del"');
    expect(h).toMatch(/class="diff__sign"[^>]*>\+</);
    expect(h).toMatch(/class="diff__sign"[^>]*>−</);
    expect(h).toContain("+2");
    expect(h).toContain("−1");
    expect(h).toContain('aria-label="Changes to s.json"');
  });
  it("collapses a 200-line change to 40 lines and offers Show all 200 lines", () => {
    const h = renderToStaticMarkup(<DiffView diff={big} label="Changes" />);
    expect(h).toContain("Show all 200 lines");
    expect((h.match(/class="diff__line"/g) ?? []).length).toBe(40);
  });
  it("counts only the changed and context lines in Show all, never the file and hunk headers", () => {
    expect(renderToStaticMarkup(<DiffView diff={big} label="Changes" />)).not.toContain("203");
  });
  it("hides the sign from a screen reader, which hears added or removed instead", () => {
    const h = renderToStaticMarkup(<DiffView diff={small} label="x" />);
    expect(h).toMatch(/class="diff__sign" aria-hidden="true"/);
    expect(h).toContain("diff__sr");
  });
  it("reads code left to right whatever its first letter, so a JSON line that starts with Hebrew is not scrambled", () => {
    const he = ["--- a/s.json", "+++ b/s.json", "@@ -1,1 +1,1 @@", '+שלום: "x"'].join("\n");
    expect(renderToStaticMarkup(<DiffView diff={he} label="x" />)).toMatch(/<code dir="ltr">/);
  });
  it("shows everything when expanded and has no button", () => {
    const h = renderToStaticMarkup(<DiffView diff={big} label="Changes" expanded />);
    expect(h).not.toContain("Show all");
    expect((h.match(/class="diff__line"/g) ?? []).length).toBe(203);
  });
  it("says so when there is nothing to change", () => {
    expect(renderToStaticMarkup(<DiffView diff="" label="Changes" />)).toContain("No changes");
  });
  it("lets names in any script read in their own direction", () => {
    expect(renderToStaticMarkup(<DiffView diff={small} label="x" />)).toContain('dir="auto"');
  });
});
