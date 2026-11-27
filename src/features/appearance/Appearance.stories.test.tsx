import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./Appearance.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the Appearance stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBe(2);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBe(2);
  });
  it("show Windows Terminal's font and schemes", () => {
    const wt = render("WithWindowsTerminal");
    expect(wt).toContain("font-family:&quot;Cascadia Code&quot;");
    expect(wt).toContain('aria-label="Add One Half Light"');
    expect(wt).toContain(">Added<");
  });
});
