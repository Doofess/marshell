import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { laneLayout, pending } from "../lane/laneLayout";
import { FIXTURES } from "../sidebar/fixtures";
import { shellLayout } from "./shellLayout";
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

describe("Sidebar: the session list scrolls", () => {
  const twenty = Array.from({ length: 20 }, (_, i) => ({ ...FIXTURES[0]!, id: `s${i}`, name: `session-${i + 1}` }));
  it("uses the real row height", () => {
    const tokens = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");
    expect(tokens).toContain(`--row-comfortable: ${ROW_HEIGHT}px`);
  });
  it("renders every session, however many: the list scrolls instead of leaving rows out", () => {
    const h = html({ rows: twenty, height: 800 });
    expect((h.match(/role="option"/g) ?? []).length).toBe(20);
    expect(h).not.toContain("more sessions");
    expect(h).not.toContain("sidebar__more");
  });
  it("makes the list itself the listbox, so the scroller and the semantics are one element", () => {
    expect(html()).toMatch(/<div class="sidebar__list" role="listbox" aria-label="Sessions">/);
  });
  it("keeps one tab stop, on the selected row", () => {
    const h = html({ rows: twenty, height: 800, selectedId: "s19" });
    expect((h.match(/role="option"[^>]*tabindex="0"|tabindex="0"[^>]*role="option"/g) ?? []).length).toBe(1);
  });
});

describe("Sidebar: the shell panel", () => {
  const withShell = (p: Partial<Parameters<typeof Sidebar>[0]> = {}) => html({ height: 880, shell: { cwd: "my-app" }, ...p });
  it("is not shown unless asked for, and then the grid has no extra row", () => {
    const h = html();
    expect(h).not.toContain('aria-label="Shell"');
    expect(h).not.toContain("data-shell");
  });
  it("sits between the session list and the footer", () => {
    const h = withShell();
    expect(h.indexOf('role="listbox"')).toBeLessThan(h.indexOf('aria-label="Shell"'));
    expect(h.indexOf('aria-label="Shell"')).toBeLessThan(h.indexOf("sidebar__footer"));
    expect(h).toContain("data-shell");
  });
  it("gets exactly the height the layout gives it, so nothing in the sidebar is clipped", () => {
    const lane = laneLayout(pending(SCENARIO_REQUESTS).length, 880);
    const expected = shellLayout(880, lane.height).height;
    const h = withShell();
    expect(h).toContain(`--shell-h:${expected}px`);
    expect(h).toContain(`block-size:${expected}px`);
  });
  it("collapses to its header by itself when the list would be squeezed", () => {
    // Twelve waiting requests take the lane's 40% share, which leaves an open panel too little room for the list.
    const many = Array.from({ length: 12 }, (_, i) => ({ ...SCENARIO_REQUESTS[0]!, id: `r${i}`, waitingMs: 1000 + i }));
    const h = withShell({ height: 480, requests: many });
    expect(h).toContain('data-collapsed="true"');
    expect(h).toContain('data-auto="true"');
  });
  it("honours the user's choice to hide it", () => {
    const h = withShell({ shell: { cwd: "my-app", collapsed: true } });
    expect(h).toContain('data-collapsed="true"');
    expect(h).not.toContain('data-auto="true"');
  });
  it("names the shell for the operating system it is given", () => {
    expect(withShell({ shell: { cwd: "my-app", shellName: "zsh" } })).toContain("zsh");
  });
});
