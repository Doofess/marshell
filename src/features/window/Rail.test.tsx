import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { Rail } from "./Rail";

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
