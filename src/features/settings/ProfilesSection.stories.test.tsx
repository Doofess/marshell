import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./ProfilesSection.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the shell profiles stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBeGreaterThanOrEqual(8);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBe(2);
  });
  it("show the core's refusal in the bar, and a quoted path in the command", () => {
    expect(render("RefusedSave")).toContain("needs a command.");
    expect(render("GitBashQuotedPath")).toContain("&quot;C:\\Program Files\\Git\\bin\\bash.exe&quot; -l");
  });
});
