import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "./fixtures";
import { SessionRow } from "./SessionRow";

describe("SessionRow", () => {
  it("has one fixture per glyph state plus the modifiers", () => {
    const kinds = new Set(FIXTURES.map((f) => f.status));
    expect(kinds.size).toBe(10);
    expect(FIXTURES.some((f) => f.muted)).toBe(true);
    expect(FIXTURES.some((f) => f.elevated)).toBe(true);
    expect(FIXTURES.some((f) => f.mode === "bypass")).toBe(true);
    expect(FIXTURES.some((f) => f.contextPct === undefined)).toBe(true);
  });
  for (const density of ["compact", "comfortable", "expanded"] as const)
    it(`renders every fixture in ${density}`, () => {
      for (const row of FIXTURES) {
        const html = renderToStaticMarkup(<SessionRow row={row} density={density} />);
        expect(html, row.id).toContain(`data-density="${density}"`);
        expect(html, row.id).toContain(`data-agent="${row.agent}"`);
        expect(html, row.id).toContain("aria-label=");
      }
    });
  it("shows line 2 only from comfortable up", () => {
    const row = FIXTURES[0]!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="compact" />)).not.toContain("session-row__phrase");
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).toContain("session-row__phrase");
  });
  it("shows usage only when expanded", () => {
    const row = FIXTURES.find((f) => f.usage)!;
    expect(renderToStaticMarkup(<SessionRow row={row} density="comfortable" />)).not.toContain(" in · ");
    expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).toContain(" in · ");
  });
  it("never puts a literal colour in markup", () => {
    for (const row of FIXTURES)
      expect(renderToStaticMarkup(<SessionRow row={row} density="expanded" />)).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});
