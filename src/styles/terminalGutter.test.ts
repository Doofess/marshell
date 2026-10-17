import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./app.css", import.meta.url), "utf8");
const view = readFileSync(new URL("../features/terminal/TerminalView.tsx", import.meta.url), "utf8");
const block = (sel: string) => {
  const i = css.indexOf(`${sel} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

// xterm.css paints .xterm-viewport #000 across xterm's padding box, so a light theme got a black gutter.
describe("terminal gutter", () => {
  it("clears xterm's black viewport so the gutter can take the theme colour", () => {
    expect(block(".terminal-host .xterm .xterm-viewport")).toContain("background-color: transparent");
  });
  it("paints the gutter with the terminal theme's background", () => {
    expect(block(".terminal-host .xterm")).toMatch(/background:\s*var\(--terminal-bg, var\(--bg-base\)\)/);
    expect(view).toContain('"--terminal-bg"');
  });
});
