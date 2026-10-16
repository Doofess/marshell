import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./Launcher.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the launcher stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBeGreaterThanOrEqual(9);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBeGreaterThanOrEqual(n === "Interactive" ? 0 : 2);
  });
  it("show the new session's number, 4, because 4 is the lowest one free", () => {
    expect(render("Empty")).toContain("Session 4");
  });
  it("show the program list with all three headings, and a shell and a tool without mode and pace chips, but with the worktree chip", () => {
    const open = render("AgentsShellsAndTools");
    for (const g of ["Agents", "Shells", "Tools"]) expect(open).toContain(`>${g}<`);
    for (const n of ["ShellPicked", "SshPicked"] as const) {
      const h = render(n);
      expect(h, n).not.toContain("Balanced");
      expect(h, n).toContain('role="checkbox"');
    }
  });
});
