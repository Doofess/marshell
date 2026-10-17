import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FIXTURES } from "../sidebar/fixtures";
import { Rail } from "./Rail";

const rows = ["needs-permission", "working", "done-unseen"].map((id) => FIXTURES.find((f) => f.id === id)!);

describe("Rail", () => {
  it("has one button per session with its vendor mark and a small status glyph, and no letters", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={1} />);
    const items = h.match(/<button[^>]*rail__item[\s\S]*?<\/button>/g)!;
    expect(items.length).toBe(3);
    for (const item of items) expect(item).toMatch(/class="agent-mark"[^>]*width="20"/);
    expect(items[0]).toMatch(/class="status-glyph"[^>]*width="12"/);
    expect(h).not.toContain("data-mono");
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
  it("marks every visible session, the focused one keeping aria-current", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={0} visibleIds={["needs-permission", "working"]} selectedId="working" />);
    const items = h.match(/<button[^>]*rail__item[^>]*>/g)!;
    expect(items.filter((i) => i.includes("data-visible")).length).toBe(2);
    expect(items[1]).toContain('aria-current="true"');
    expect(items[1]).toContain("data-visible");
    expect(items[0]).not.toContain("aria-current");
    expect(items[2]).not.toContain("data-visible");
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} selectedId="working" />)).not.toContain("data-visible");
  });
  it("does not mark the store's selection when it is in no visible pane", () => {
    expect(renderToStaticMarkup(<Rail rows={rows} needsYou={0} visibleIds={["working"]} selectedId="needs-permission" />)).not.toContain("aria-current");
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
  it("keeps the marks out of the button's name: the label says the session, the marks are pictures of it", () => {
    const h = renderToStaticMarkup(<Rail rows={rows} needsYou={0} />);
    expect(h).toContain('aria-label="Session 1, ');
    expect(h.match(/<span class="rail__mark" aria-hidden="true">/g)?.length).toBe(3);
  });
});

describe("Rail: the list scrolls", () => {
  const twenty = Array.from({ length: 20 }, (_, i) => ({ ...FIXTURES[0]!, id: `s${i}`, name: `session-${i + 1}` }));
  it("renders every session, however many, and no overflow button", () => {
    const h = renderToStaticMarkup(<Rail rows={twenty} needsYou={0} />);
    expect((h.match(/class="rail__item[ "]/g) ?? []).length).toBe(20);
    expect(h).not.toContain("rail__more");
    expect(h).not.toContain("show all");
  });
  it("has no palette tool: the top bar keeps it", () => {
    const h = renderToStaticMarkup(<Rail rows={twenty} needsYou={2} />);
    expect(h).not.toContain("rail__tools");
    expect(h).not.toContain("Command palette");
  });
  it("keeps the needs-you badge stack inside the list, at its top", () => {
    const h = renderToStaticMarkup(<Rail rows={twenty} needsYou={2} />);
    expect(h.indexOf("rail__badge")).toBeGreaterThan(h.indexOf('class="rail__list"'));
    expect(h.indexOf("rail__badge")).toBeLessThan(h.indexOf("rail__item"));
  });
});
