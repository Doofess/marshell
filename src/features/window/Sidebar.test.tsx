import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { laneLayout, pending } from "../lane/laneLayout";
import { FIXTURES } from "../sidebar/fixtures";
import { listLayout } from "./listLayout";
import { Sidebar, ROW_HEIGHT } from "./Sidebar";

const html = (p: Partial<Parameters<typeof Sidebar>[0]> = {}) =>
  renderToStaticMarkup(<Sidebar rows={SCENARIO_ROWS} requests={SCENARIO_REQUESTS} height={760} width={288} footer={SCENARIO_FOOTER} {...p} />);

describe("Sidebar", () => {
  it("stacks the lane above the session list above a 40 px footer", () => {
    const h = html();
    expect(h.indexOf('class="lane"')).toBeLessThan(h.indexOf('role="listbox"'));
    expect(h.indexOf('role="listbox"')).toBeLessThan(h.indexOf("sidebar__footer"));
  });
  it("lists every session as an option with one tab stop", () => {
    const h = html({ selectedId: "working" });
    expect((h.match(/role="option"/g) ?? []).length).toBe(5);
    expect((h.match(/tabindex="0"/g) ?? []).length).toBeGreaterThanOrEqual(1);
    expect(h).toContain('aria-selected="true"');
  });
  it("shows the plan ring, the 5h percentage and the doctor count in the footer", () => {
    const h = html();
    expect(h).toContain("5h 38%");
    expect(h).toContain("Doctor: 2 issues");
  });
  it("leaves out unknown plan usage, and shows a clean doctor as such", () => {
    const h = html({ footer: {} });
    expect(h).not.toContain("sidebar__plan");
    expect(h).not.toContain("5h ");
    expect(h).not.toContain("–");
    expect(h).toContain("Doctor: no issues");
  });
  it("keeps the lane header when nothing waits", () => {
    expect(html({ requests: [] })).toContain("All clear");
  });
  it("sets its own width", () => {
    expect(html({ width: 300 })).toContain("inline-size:300px");
  });
});

describe("Sidebar: it never scrolls", () => {
  const twenty = Array.from({ length: 20 }, (_, i) => ({ ...FIXTURES[0]!, id: `s${i}`, name: `session-${i + 1}` }));
  const list = (h: string) => h.slice(h.indexOf('class="sidebar__list"'), h.indexOf("sidebar__footer"));
  it("uses the real row height", () => {
    const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");
    expect(tokens).toContain(`--row-comfortable: ${ROW_HEIGHT}px`);
  });
  it("shows the rows that fit the space under the lane, and a button for the rest", () => {
    const h = html({ rows: twenty, height: 800 });
    const lane = laneLayout(pending(SCENARIO_REQUESTS).length, 800);
    const { visible, hidden } = listLayout(20, 800 - lane.height - 40, ROW_HEIGHT);
    expect((h.match(/role="option"/g) ?? []).length).toBe(visible);
    expect(hidden).toBeGreaterThan(0);
    expect(h).toMatch(/aria-label="\+\d+ more sessions, show all 20"/);
    expect(h).toContain(`+${hidden} more sessions`);
  });
  it("shows no button when everything fits", () => {
    expect(html()).not.toMatch(/Show all d+ sessions/);
  });
  it("keeps the button outside the listbox, where only options belong", () => {
    const h = list(html({ rows: twenty, height: 800 }));
    const box = h.slice(h.indexOf('role="listbox"'), h.indexOf("</div>", h.lastIndexOf('role="option"')));
    expect(box).not.toContain("<button");
    expect(h.indexOf("sidebar__more")).toBeGreaterThan(h.lastIndexOf('role="option"'));
  });
  it("leaves no hidden row in the tab order: the one tab stop is on a row that is on screen", () => {
    const h = html({ rows: twenty, height: 800, selectedId: "s19" });
    expect((h.match(/tabindex="0"/g) ?? []).length).toBeGreaterThanOrEqual(1);
    expect(h).not.toContain("session-20");
  });
});
