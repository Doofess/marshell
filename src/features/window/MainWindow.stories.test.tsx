import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./MainWindow.stories";

const render = (name: keyof typeof stories) => renderToStaticMarkup(((stories[name] as unknown as { render: () => React.ReactElement }).render)());

describe("the empty window story", () => {
  it("shows the first-run state in dark and light: no terminal, no prompt line, and how to start a session", () => {
    const h = render("NoSessions");
    expect((h.match(/No sessions yet\. Start one with Ctrl\+Shift\+T\./g) ?? []).length).toBe(2);
    expect(h).not.toContain('aria-label="Terminal for');
    expect(h).not.toContain('<form class="composer"');
  });
});

describe("the shell panel stories", () => {
  it("show the sidebar with the panel open, in dark and light", () => {
    const h = render("SidebarWithShell");
    expect((h.match(/<section class="shell-panel"/g) ?? []).length).toBe(2);
    expect(h).toContain('aria-label="PowerShell terminal"');
  });
  it("show it hidden by the user, header only", () => {
    const h = render("ShellHidden");
    expect(h).toContain('data-collapsed="true"');
    expect(h).not.toContain('data-auto="true"');
  });
  it("show it folding itself away in a short window", () => {
    const h = render("ShellFoldsInShortWindow");
    expect(h).toContain('data-auto="true"');
  });
});
