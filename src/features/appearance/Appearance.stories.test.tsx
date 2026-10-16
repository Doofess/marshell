import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./Appearance.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the Appearance stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBe(3);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBe(2);
  });
  it("show the refused accent's reason, and Windows Terminal's font and schemes", () => {
    expect(render("CustomAccentRefused")).toContain("Too light for the light theme");
    const wt = render("WithWindowsTerminal");
    expect(wt).toContain("From Windows Terminal: Cascadia Code");
    expect(wt).toContain('aria-label="Add One Half Light"');
    expect(wt).toContain(">Added<");
  });
});
