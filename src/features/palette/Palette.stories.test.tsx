import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./Palette.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the palette stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBeGreaterThanOrEqual(10);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBe(n === "Interactive" ? 0 : 2);
  });
  it("show session 6 first for 6, and say which numbers exist for one nobody holds", () => {
    expect(render("SessionSix")).toContain("Go to session 6, PowerShell");
    expect(render("NoSuchSession")).toContain("No session 4. Open sessions: 1, 2, 3, 5, 6, 7, 8, 9, 12.");
  });
  it("show a manual CLI session by its mark and number, past nine without a shortcut", () => {
    const h = render("SessionPastNine");
    expect(h).toContain("Go to session 12, load-test");
    expect(h).toContain('data-agent="python"');
    expect(h).not.toContain("Ctrl+12");
  });
  it("show the plain command list when no sessions are open", () => {
    expect(render("CommandsOnly")).not.toContain("Open sessions");
  });
});
