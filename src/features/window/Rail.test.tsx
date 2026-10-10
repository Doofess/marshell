import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { listLayout } from "./listLayout";
import { RAIL_ITEM, RAIL_BADGE, Rail } from "./Rail";

const rows = ["needs-permission", "working", "done-unseen"].map((id) => FIXTURES.find((f) => f.id === id)!);

describe("Rail", () => {
  it("has one button per session with the monogram and the status glyph", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={1} />);
    expect((h.match(/class="rail__item"/g) ?? []).length).toBe(3);
    expect(h).toContain("BI");
    expect(h).toContain("AS");
    expect(h).toContain("status-glyph");
  });
  it("labels each button with the same text the row has", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={1} />);
    expect(h).toContain("Claude Sonnet 5.5, Needs you: permission");
    expect(h).toContain("billing ");
  });
  it("stacks a needs-you badge with a count at the top, only when something waits", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={2} />)).toContain("rail__badge");
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} />)).not.toContain("rail__badge");
  });
  it("marks the selected session", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} selectedId="working" />)).toContain('aria-current="true"');
  });
  it("carries the agent so the brand stripe colours", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} />)).toContain('data-agent="claude"');
  });
  it("shows a working session as its animated vendor mark, not a glyph (the user's answer: the rail uses the mark)", () => {
    const working = FIXTURES.find((f) => f.id === "working")!;
    const h = renderToStaticMarkup(<Rail rows={[working]} needsYou={0} />);
    expect(h).toContain("agent-mark");
    expect(h).toContain("data-working");
    expect(h).not.toContain('data-kind="working"');
    const other = renderToStaticMarkup(<Rail rows={[FIXTURES.find((f) => f.id === "done-unseen")!]} needsYou={0} />);
    expect(other).toContain('data-kind="done-unseen"');
  });
});

describe("Rail (accessibility gate)", () => {
  it("draws the monogram from an attribute, so the button's name is not required to repeat letters that are only decoration", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={0} />);
    expect(h).toContain('data-mono="BI"');
    expect(h).not.toContain("rail__mono");
  });
});

describe("Rail: it never scrolls", () => {
  const twenty = Array.from({ length: 20 }, (_, i) => ({ ...FIXTURES[0]!, id: `s${i}`, name: `session-${i + 1}` }));
  it("shows the sessions that fit between the bar and the tools, and a +N button for the rest", () => {
    const h = renderToStaticMarkup(<Rail rows={twenty} needsYou={0} height={800} />);
    const { visible, hidden } = listLayout(20, 800 - 40 - 40 - 16, RAIL_ITEM);
    expect((h.match(/class="rail__item"/g) ?? []).length).toBe(visible);
    expect(h).toMatch(/aria-label="\+\d+, show all 20 sessions"/);
    expect(h).toContain(`+${hidden}`);
  });
  it("gives the needs-you badge stack its room before counting rows", () => {
    const without = renderToStaticMarkup(<Rail rows={twenty} needsYou={0} height={800} />);
    const withBadge = renderToStaticMarkup(<Rail rows={twenty} needsYou={2} height={800} />);
    expect((withBadge.match(/class="rail__item"/g) ?? []).length).toBeLessThan((without.match(/class="rail__item"/g) ?? []).length + 1);
    expect(RAIL_BADGE).toBeGreaterThan(0);
  });
  it("shows no button when the sessions fit", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} height={800} />)).not.toContain("Show all");
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} />)).not.toContain("Show all");
  });
});
