import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import * as stories from "./ReviewSheet.stories";

type Story = { render: () => React.ReactElement };
const names = Object.keys(stories).filter((k) => k !== "default") as Array<keyof typeof stories>;
const render = (name: keyof typeof stories) => renderToStaticMarkup((stories[name] as unknown as Story).render());

describe("the Review sheet stories", () => {
  it("render, each in a dark and a light panel", () => {
    expect(names.length).toBe(5);
    for (const n of names) expect((render(n).match(/class="theme-pair__panel"/g) ?? []).length, n).toBe(2);
  });
  it("show what each case is about", () => {
    expect(render("FileSelected")).toContain("check(password)");
    expect(render("BinaryFile")).toContain("This is a binary file");
    expect(render("Truncated")).toContain("The diff is too large to show in full.");
    expect(render("Empty")).toContain("No changes yet.");
  });
});
