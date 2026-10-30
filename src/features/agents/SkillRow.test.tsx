import { renderToStaticMarkup } from "react-dom/server";
import type { AgentSetup } from "../../generated/AgentSetup";
import { describe, expect, it, vi } from "vitest";
import { PREVIEW, SETUP } from "./fixtures";
import { SkillRow } from "./SkillRow";
import type { SkillFlow } from "./useSkillFlow";

const on = { install: vi.fn(), remove: vi.fn(), apply: vi.fn(), toggleDiff: vi.fn(), reload: vi.fn(), cancel: vi.fn() };
const html = (skill: AgentSetup["skill"], flow: SkillFlow | null = null, skill_problem?: string) => renderToStaticMarkup(<SkillRow agent="claude" setup={{ ...SETUP, skill, skill_problem }} flow={flow} on={on} />);
const text = (h: string) => h.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();

describe("SkillRow", () => {
  it("none: says it is not installed and offers the install", () => {
    const h = html("none");
    expect(h).toContain('aria-label="marshell skill"');
    expect(text(h)).toBe("Not installed. Install the marshell skill");
  });
  it("installed: shows the path and offers the removal", () => {
    expect(text(html("installed"))).toBe(`Installed in ${SETUP.skill_path}. Remove the marshell skill`);
  });
  it("outdated: says a newer version is ready", () => {
    expect(text(html("outdated"))).toBe("A newer version is ready. Update the marshell skill");
  });
  it("edited: says it changed and that updating shows what changes", () => {
    expect(text(html("edited"))).toBe("Changed since Marshell wrote it. Updating shows what changes. Update the marshell skill");
  });
  it("unavailable: its line is the problem and there is no button", () => {
    const h = html("unavailable", null, "Marshell will not edit X: No.");
    expect(text(h)).toBe("Marshell will not edit X: No.");
    expect(h).not.toContain("<button");
  });
  it("an open flow shows the diff toggle and Apply", () => {
    const h = html("none", { kind: "install", preview: PREVIEW, open: true, busy: false });
    expect(h).toContain("View exact changes");
    expect(h).toContain("Apply");
    expect(h).toContain('aria-expanded="true"');
  });
  it("a busy flow disables Apply; a conflict offers Reload; an error is shown", () => {
    expect(html("none", { kind: "install", preview: PREVIEW, open: false, busy: true })).toMatch(/<button[^>]*disabled[^>]*>Apply/);
    expect(html("none", { kind: "install", preview: PREVIEW, open: false, busy: false, conflict: true })).toContain("Reload");
    expect(html("none", { kind: "install", preview: null, open: false, busy: false, error: "No." })).toContain("No.");
  });
  it("adds no outline or border of its own", () => {
    expect(html("none", { kind: "install", preview: PREVIEW, open: true, busy: false })).not.toMatch(/outline|border/);
  });
});
