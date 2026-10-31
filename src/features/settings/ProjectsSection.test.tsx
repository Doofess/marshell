import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ProjectInfo } from "../../generated/ProjectInfo";
import { ProjectsView, type ProjectsHandlers, type ProjectsState } from "./ProjectsSection";

const noop = () => {};
const handlers: ProjectsHandlers = { onSelect: noop, onToggle: noop, onPatch: noop, onAdd: noop, onRemove: noop, onSave: noop };
const P: ProjectInfo[] = [{ path: "C:\\dev\\app", name: "app", branch: null, last_used_ms: 1 }];
const state = (over: Partial<ProjectsState> = {}): ProjectsState => ({ phase: "ready", selected: "C:\\dev\\app", enabled: false, rows: [], error: null, saving: false, ...over });
const html = (s: ProjectsState, projects = P) => renderToStaticMarkup(<ProjectsView projects={projects} state={s} handlers={handlers} />);

describe("ProjectsView", () => {
  it("uses the copy table's texts", () => {
    const h = html(state({ rows: [{ name: "login", path: "src/login.ts" }] }));
    for (const t of ["Project", "Add references to prompts", "When a prompt names an alias, Marshell tells the agent which file or folder it means. Off until you turn it on.", "Alias", "Path in the project", "Add alias", "Save"]) expect(h).toContain(t);
    expect(h).toContain("C:\\dev\\app");
  });
  it("labels each remove button with its alias", () => {
    expect(html(state({ rows: [{ name: "login", path: "a" }] }))).toContain('aria-label="Remove login"');
  });
  it("says there are no aliases, and no projects", () => {
    expect(html(state())).toContain("No aliases yet.");
    const h = html(state({ selected: null, phase: "idle" }), []);
    expect(h).toContain("Start a session in a folder first; its project shows here.");
    expect(h).not.toContain("Add alias");
  });
  it("shows the broken line beside a path that was not found", () => {
    const h = html(state({ rows: [{ name: "a", path: "x", exists: false }, { name: "b", path: "y", exists: true }] }));
    expect(h.split("Not found in the project, so it isn&#x27;t sent.").length - 1).toBe(1);
  });
  it("shows a refusal under the list, focusable", () => {
    const h = html(state({ error: 'Two aliases are named "a".' }));
    expect(h).toContain("Two aliases are named &quot;a&quot;.");
    expect(h).toContain('tabindex="-1"');
  });
  it("has no live region of its own", () => {
    expect(html(state({ error: "x" }))).not.toMatch(/aria-live|role="status"|role="alert"/);
  });
  // The Antigravity line ("Antigravity can't take references yet.") waits on the spike; see ProjectsSection.tsx.
});
