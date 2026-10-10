import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { Sidebar } from "./Sidebar";

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
