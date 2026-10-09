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

describe("SessionRow (final review)", () => {
  const row = FIXTURES.find((f) => f.id === "needs-permission")!;
  it("is an option in a listbox with roving focus", () => {
    const sel = renderToStaticMarkup(<SessionRow row={row} density="compact" selected tabStop />);
    expect(sel).toContain('role="option"');
    expect(sel).toContain('aria-selected="true"');
    expect(sel).toContain('tabindex="0"');
    const other = renderToStaticMarkup(<SessionRow row={row} density="compact" />);
    expect(other).toContain('aria-selected="false"');
    expect(other).toContain('tabindex="-1"');
  });
  it("marks muted rows so their badge does not bounce", () => {
    expect(renderToStaticMarkup(<SessionRow row={{ ...row, muted: true }} density="compact" />)).toContain("data-muted");
  });
  it("lets the browser pick the direction of names, projects and branches", () => {
    const html = renderToStaticMarkup(<SessionRow row={FIXTURES.find((f) => f.id === "rtl")!} density="compact" />);
    expect(html.match(/dir="auto"/g)?.length).toBeGreaterThanOrEqual(3);
  });
  it("renders no empty branch head, so the tail never reads as a whole word", () => {
    const html = renderToStaticMarkup(<SessionRow row={{ ...row, branch: "main" }} density="compact" />);
    expect(html).not.toContain("session-row__branch-head");
    expect(html).toContain("session-row__branch-tail");
  });
  it("has fixtures for the crowded cases at 200 px", () => {
    expect(FIXTURES.some((f) => f.id === "crowded")).toBe(true);
    expect(FIXTURES.some((f) => f.id === "emoji-name")).toBe(true);
    expect(FIXTURES.some((f) => f.id === "muted-needs-you" && f.muted && f.status === "needs-permission")).toBe(true);
  });
});
