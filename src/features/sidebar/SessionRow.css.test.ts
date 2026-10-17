import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./SessionRow.css", import.meta.url), "utf8");
const rule = (selector: string) => {
  const i = css.indexOf(`${selector} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("SessionRow.css (final review)", () => {
  it("dims an ended row with --text-3, which passes AA, across the whole row", () => {
    expect(rule(".session-row[data-ended]")).toContain("var(--text-3)");
    expect(css).not.toMatch(/--text-1\)\s*60%/);
  });
  it("draws selection as an overlay so it layers with the needs-you tint", () => {
    expect(rule('.session-row[aria-selected="true"]')).toContain("box-shadow");
  });
  it("clips the lead of line 1 so the right cluster can never be pushed out", () => {
    expect(rule(".session-row__lead")).toMatch(/overflow:\s*clip/);
    expect(rule(".session-row__lead")).toContain("min-inline-size: 0");
  });
  it("keeps a visible ellipsis before the branch tail", () => {
    expect(rule(".session-row__branch-head")).toMatch(/min-inline-size:\s*1\.5ch/);
  });
  it("lets the branch tail shrink after the project", () => {
    expect(rule(".session-row__branch-tail")).toMatch(/flex:\s*0 50 auto/);
  });
  it("uses tabular figures across the row", () => {
    expect(rule(".session-row")).toContain("tabular-nums");
  });
});

describe("SessionRow.css (blocked rows)", () => {
  it("adds no second edge line: the brand stripe is the only mark at the edge", () => {
    expect(css).not.toContain("::before");
    expect(css).not.toContain("::after");
  });
  it("tints every blocked row with the vendor's own colour, so a list stays one family of colours", () => {
    expect(rule(".session-row[data-blocked]")).toContain("var(--signal-tint)");
    expect(css).not.toContain("var(--accent-tint)");
    expect(css).not.toMatch(/color-mix\(in oklch, var\(--error\)/);
  });
  it("derives the tint and the flash from the vendor signal, falling back to the accent", () => {
    const r = rule(".session-row");
    expect(r).toContain("--signal: var(--agent-signal, var(--accent))");
    expect(r).toMatch(/--signal-tint:\s*color-mix\(in oklch, var\(--signal\) 12%/);
    expect(r).toMatch(/--signal-flash:\s*color-mix\(in oklch, var\(--signal\) 24%/);
  });
  it("shows the reason line in full-contrast text", () => {
    expect(rule(".session-row[data-blocked] .session-row__phrase")).toContain("var(--text-1)");
  });
  it("draws the focused row's bar on the inline end in the accent, with logical properties only", () => {
    const r = rule(".session-row__bar");
    expect(r).toContain("3px");
    expect(r).toContain("var(--accent)");
    expect(css).not.toMatch(/(^|[\s;{])(left|right)\s*:/m);
  });
  it("makes that bar a short rounded pill set in from the row's edge, so it never runs into the focused pane's ring across the seam", () => {
    const r = rule(".session-row__bar");
    expect(r).toMatch(/inset-inline-end:\s*(?!0[;\s])[^;]+/);
    expect(r).toContain("block-size: 16px");
    expect(r).toContain("margin-block: auto");
    expect(r).toMatch(/border-radius:\s*(?!0[;\s])[^;]+/);
  });
  it("tells visible and focused rows apart by outline shape in forced colours", () => {
    const fc = css.slice(css.indexOf("@media (forced-colors: active)"));
    const r = (sel: string) => {
      const j = fc.indexOf(`${sel} {`);
      return j < 0 ? "" : fc.slice(j, fc.indexOf("}", j));
    };
    expect(r('.session-row[aria-selected="true"]')).toContain("1px dashed Highlight");
    expect(r(".session-row[data-focused]")).toContain("2px solid Highlight");
  });
});
