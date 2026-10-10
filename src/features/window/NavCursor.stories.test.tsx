import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./NavCursor.stories";

type Story = { render: () => React.ReactElement };
const entries = Object.entries(stories).filter(([k]) => k !== "default") as unknown as Array<[string, Story]>;
const byName = (n: string) => entries.find(([k]) => k === n)![1];

describe("keyboard navigation stories", () => {
  it("has the stories the review needs", () => {
    expect(entries.map(([k]) => k)).toEqual(["NoneSelected", "TabSelectsTheNextTerminal", "HeldOnASidebarRow", "HeldOnASessionNotOnScreen", "HeldAcrossPanes", "HeldOnTheTopBar", "MacOs", "ShortcutsWindows", "ShortcutsMac", "ShortcutsLinux"]);
  });
  for (const [name, story] of entries) it(`${name} renders`, () => expect(() => renderToStaticMarkup(story.render())).not.toThrow());
  it("draws a cursor only while the modifier is held on a target", () => {
    const h = (n: string) => renderToStaticMarkup(byName(n).render());
    expect(h("NoneSelected")).not.toContain('class="nav-cursor"');
    expect(h("NoneSelected")).toContain("No session selected · Tab to select a terminal");
    expect(h("TabSelectsTheNextTerminal")).not.toContain('class="nav-cursor"');
    for (const n of ["HeldOnASidebarRow", "HeldOnASessionNotOnScreen", "HeldAcrossPanes", "HeldOnTheTopBar", "MacOs"]) expect(h(n), n).toContain('class="nav-cursor"');
    expect(h("HeldOnASessionNotOnScreen")).toContain("Session 5, docs-site");
  });
});
