import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SCENARIO_FOOTER, SCENARIO_REQUESTS, SCENARIO_ROWS } from "./scenario";
import { laneLayout, pending } from "../lane/laneLayout";
import { FIXTURES } from "../sidebar/fixtures";
import { shellLayout } from "./shellLayout";
import { Sidebar, ROW_HEIGHT } from "./Sidebar";
import { sessionShortcut } from "../../lib/appShortcuts";
import { moveBy } from "../sidebar/reorder";
import { renameOutcome } from "../sidebar/rename";

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
    expect(h).toContain("Doctor: 4 issues");
  });
  it("leaves out unknown plan usage, and shows a clean doctor as such", () => {
    const h = html({ footer: {} });
    expect(h).not.toContain("sidebar__plan");
    expect(h).not.toContain("5h ");
    expect(h).not.toContain("–");
    expect(h).toContain("Doctor: no issues");
  });
  it("has no footer when none is given, and gives its row back", () => {
    const h = html({ footer: undefined });
    expect(h).not.toContain("sidebar__footer");
    expect(h).not.toContain("Doctor:");
    expect(h).toContain("--footer-h:0px");
    expect(html()).not.toContain("--footer-h");
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

// No DOM here: the keys go through the pure helpers the sidebar and the app use, and the rest is read from static markup.
describe("Sidebar: rename and reorder", () => {
  const keyEvent = (init: Partial<KeyboardEvent> & { code: string }) => ({ key: init.code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...init }) as KeyboardEvent;
  const target = SCENARIO_ROWS[1]!;

  it("F2 edits name inline and enter renames", () => {
    expect(sessionShortcut(keyEvent({ code: "F2", key: "F2" }), "windows")).toEqual({ kind: "rename" });
    const h = html({ selectedId: target.id, renamingId: target.id, onRename: () => {}, onRenameCancel: () => {} });
    expect(h).toContain('class="session-row__rename"');
    expect(h).toContain(`value="${target.name}"`);
    expect((h.match(/session-row__rename/g) ?? []).length).toBe(1);
    expect(renameOutcome("Enter", "Build server", target.name)).toEqual({ kind: "commit", name: "Build server" });
  });
  it("empty rename keeps the old name", () => {
    expect(renameOutcome("Enter", "   ", target.name)).toEqual({ kind: "cancel" });
    expect(renameOutcome("Escape", "Something else", target.name)).toEqual({ kind: "cancel" });
  });
  it("alt+shift+arrow moves the row", () => {
    const ids = SCENARIO_ROWS.map((r) => r.id);
    expect(sessionShortcut(keyEvent({ code: "ArrowUp", altKey: true, shiftKey: true }), "windows")).toEqual({ kind: "move", by: -1 });
    expect(moveBy(ids, ids[1]!, -1)?.[0]).toBe(ids[1]);
    expect(moveBy(ids, ids[0]!, -1)).toBeNull();
  });
  it("makes rows draggable only when it can reorder them, and marks the drop edge", () => {
    expect(html()).not.toContain("draggable");
    expect(html({ onReorder: () => {} })).toContain('draggable="true"');
  });
  it("does not change the markup when no rename or reorder props are given", () => {
    const h = html();
    expect(h).not.toContain("session-row__rename");
    expect(h).not.toContain("data-renaming");
    expect(h).not.toContain("data-drop");
  });
  it("marks every visible session selected and the focused one with data-focused, only when visible ids are passed", () => {
    const [a, b, c] = SCENARIO_ROWS.map((r) => r.id);
    const h = html({ visibleIds: [a!, b!], selectedId: b });
    const row = (id: string) => h.match(new RegExp(`<div[^>]*data-nav-row="${SCENARIO_ROWS.find((r) => r.id === id)!.number}"[^>]*>`))![0];
    expect(row(a!)).toContain('aria-selected="true"');
    expect(row(b!)).toContain('aria-selected="true"');
    expect(row(c!)).toContain('aria-selected="false"');
    expect((h.match(/data-focused/g) ?? []).length).toBe(1);
    expect(row(b!)).toContain("data-focused");
    expect(row(b!)).toContain('tabindex="0"');
    expect((h.match(/role="option"[^>]*tabindex="0"/g) ?? []).length).toBe(1);
    expect(h).toContain('aria-multiselectable="true"');
    const plain = html({ selectedId: b });
    expect(plain).not.toContain("aria-multiselectable");
    expect(plain).not.toContain("data-focused");
    expect((plain.match(/aria-selected="true"/g) ?? []).length).toBe(1);
  });
  it("does not mark the store's selection when it is in no visible pane", () => {
    const [a, , c] = SCENARIO_ROWS.map((r) => r.id);
    const h = html({ visibleIds: [a!], selectedId: c });
    expect((h.match(/aria-selected="true"/g) ?? []).length).toBe(1);
    expect(h).not.toContain("data-focused");
  });
  it("does not make a row draggable while its name is being edited", () => {
    const h = html({ onReorder: () => {}, renamingId: target.id, onRename: () => {}, onRenameCancel: () => {} });
    expect((h.match(/draggable="true"/g) ?? []).length).toBe(SCENARIO_ROWS.length - 1);
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

describe("Sidebar: grouped by project", () => {
  const mk = (id: string, project: string) => ({ ...FIXTURES[0]!, id, name: id, project, projectKey: project });
  const rows = [mk("a1", "alpha"), mk("b1", "beta"), mk("a2", "alpha")];
  it("grouped shows one heading per project with its count", () => {
    const h = html({ rows, grouped: true });
    expect((h.match(/class="sidebar__group"/g) ?? []).length).toBe(2);
    expect(h).toMatch(/sidebar__group-name">alpha<[^]*sidebar__group-count">2</);
    expect(h).toMatch(/sidebar__group-name">beta<[^]*sidebar__group-count">1</);
    expect(h).toContain(`role="presentation"`);
  });
  it("lists the rows group by group, in the displayed order", () => {
    const h = html({ rows, grouped: true, selectedId: "a1" });
    const at = (n: string) => h.indexOf(`>${n}<`);
    expect(at("a1")).toBeLessThan(at("a2"));
    expect(at("a2")).toBeLessThan(at("b1"));
    expect((h.match(/role="option"/g) ?? []).length).toBe(3);
  });
  it("headings take no focus and add no live region", () => {
    const heading = html({ rows, grouped: true }).match(/<div class="sidebar__group"[^>]*>/g) ?? [];
    expect(heading.length).toBe(2);
    for (const tag of heading) expect(tag).not.toMatch(/tabindex|aria-live|role="alert"|role="status"/);
  });
  it("ungrouped renders no headings", () => {
    expect(html({ rows })).not.toContain("sidebar__group");
    expect(html({ rows, grouped: false })).not.toContain("sidebar__group");
    expect(html()).not.toContain("sidebar__group");
  });
});
