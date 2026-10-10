import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { labelOf, nameContainsLabel } from "../../lib/labelInName";
import { SCENARIO_ROWS_WITH_TOOLS } from "../window/scenario";
import { FIXTURES } from "../sidebar/fixtures";
import { DeparturesBoard } from "./DeparturesBoard";

const html = (o: Partial<Parameters<typeof DeparturesBoard>[0]> = {}) => renderToStaticMarkup(<DeparturesBoard rows={SCENARIO_ROWS_WITH_TOOLS} heightPx={800} {...o} />);

describe("DeparturesBoard", () => {
  it("has the plan's seven columns as real column headers", () => {
    const h = html();
    for (const c of ["STATUS", "SESSION", "PROJECT / BRANCH", "AGENT", "WAITING", "CTX", "RECAP"]) expect(h).toMatch(new RegExp(`<th[^>]*scope="col"[^>]*>${c.replace("/", "\\/")}`));
  });
  it("puts Gate calls first and counts them", () => {
    const h = html();
    expect(h.indexOf("Gate calls")).toBeGreaterThan(-1);
    expect(h.indexOf("Gate calls")).toBeLessThan(h.indexOf('data-gate="false"'));
    expect(h).toMatch(/Gate calls · 2/);
  });
  it("names every row from its visible text, which carries the session number (read as Session N)", () => {
    const rows = html().match(/<tr class="board__row[^"]*"[\s\S]*?<\/tr>/g) ?? [];
    expect(rows.length).toBeGreaterThan(5);
    for (const row of rows) {
      // Named from its content: an aria-label cannot both start with the session and contain the text in column order.
      expect(labelOf(row)).toBeNull();
      expect(nameContainsLabel(row)).toBe(true);
      expect(row).toMatch(/<span class="board__sr">Session <\/span>/);
      expect(row).toMatch(/title="Session \d+, /);
    }
  });
  it("marks the active row with the shared selected mark", () => {
    expect(html({ active: 1 })).toContain("is-selected-mark");
    expect(html({ active: 1 })).toContain('aria-current="true"');
  });
  it("shows a plain terminal as RUNNING with an en dash for context", () => {
    const h = html();
    expect(h).toContain("RUNNING");
    expect(h).toContain("–");
  });
  it("never lets twelve sessions overflow: it counts the rest and offers the all-sessions menu", () => {
    const twelve = Array.from({ length: 12 }, (_, i) => ({ ...FIXTURES[2]!, id: `s${i}`, number: i + 1 }));
    const h = renderToStaticMarkup(<DeparturesBoard rows={twelve} heightPx={400} />);
    expect(h).toMatch(/\+\d+ more sessions/);
    expect(h).toContain("All sessions");
  });
  it("says so when there are no sessions", () => {
    expect(renderToStaticMarkup(<DeparturesBoard rows={[]} heightPx={400} />)).toContain("No sessions on the board");
  });
  it("opens the approve card inline under a row that needs you", () => {
    const gate = SCENARIO_ROWS_WITH_TOOLS.find((r) => r.status === "needs-permission")!;
    expect(html({ card: gate.id })).toContain("approve-card");
    expect(html()).not.toContain("approve-card");
  });
  it("lets names read in their own direction and has no live region", () => {
    expect(html()).toContain('dir="auto"');
    expect(html()).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  it("plays a whole clip on a cell from a track of frames, and shows the track's last text", () => {
    const h = html({ tracks: { working: { status: [{ t: 0, text: "WORKING" }, { t: 1000, text: "NEEDS YOU" }] } } });
    expect(h).toMatch(/data-frames="\[\{[^"]*NEEDS YOU[^"]*"[^>]*data-at="0"/);
    expect(h).toMatch(/>NEEDS YOU<\/span>/);
  });
  it("leaves a cell with no track and no flip as plain text", () => {
    expect(html().match(/class="flip"/g)).toBeNull();
  });
  it("can show one frozen moment of the clip: the given texts replace the row's own", () => {
    const h = html({ texts: { working: { status: "NEEDS YOU", waiting: "<1 MIN" } } });
    const row = (h.match(/<tr class="board__row[^"]*"[^>]*data-agent="claude"[\s\S]*?<\/tr>/g) ?? []).find((r) => r.includes("NEEDS YOU") && r.includes("&lt;1 MIN"));
    expect(row).toBeDefined();
  });
});
