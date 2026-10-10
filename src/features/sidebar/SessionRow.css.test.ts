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
  it("draws a 3 px accent band inside the brand stripe for needs-you", () => {
    const r = rule('.session-row[data-blocked="needs-you"]::before');
    expect(r).toContain("inline-size: 3px");
    expect(r).toContain("var(--accent)");
  });
  it("draws the same band in the error colour for error", () => {
    expect(rule('.session-row[data-blocked="error"]::before')).toContain("var(--error)");
  });
  it("shows the reason line in full-contrast text", () => {
    expect(rule(".session-row[data-blocked] .session-row__phrase")).toContain("var(--text-1)");
  });
  it("tints the error row with the error colour", () => {
    expect(rule('.session-row[data-blocked="error"]')).toContain("var(--error)");
  });
  it("keeps the band visible in Windows High Contrast", () => {
    expect(css.slice(css.indexOf("@media (forced-colors: active)"))).toContain("Highlight");
  });
});
