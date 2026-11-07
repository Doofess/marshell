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
  it("show a typed path that exists and one that doesn't", () => {
    expect(render("TypedPathFound")).toContain("Use");
    expect(render("TypedPathMissing")).toContain('aria-disabled="true"');
  });
  it("show the shell list, the agent list led by Just the shell, and Just the shell without mode and pace chips but with the worktree chip", () => {
    expect(render("Shells")).toContain('aria-label="Shells"');
    const agents = render("AgentsInTheShell");
    expect(agents).toContain('aria-label="Agents in PowerShell"');
    expect(agents).toContain(">Just the shell<");
    for (const n of ["ShellPicked"] as const) {
      const h = render(n);
      expect(h, n).not.toContain("Balanced");
      expect(h, n).toContain('role="checkbox"');
    }
  });
  it("show the missing line, the checking note and the unreached note", () => {
    expect(render("AgentMissingInTheShell")).toContain("Not in WSL: Antigravity (not installed), GitHub Copilot (Marshell runs it in WSL later)");
    expect(render("CheckingTheShell")).toContain("Checking which agents WSL has…");
    expect(render("ShellUnreached")).toContain("so only the shell is offered.");
  });
});
