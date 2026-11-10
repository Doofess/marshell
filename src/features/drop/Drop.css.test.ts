import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Drop.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (selector: RegExp) => new RegExp(`${selector.source}\\s*\\{([^}]*)\\}`);

describe("Drop.css", () => {
  it("frames a lone terminal or the side pane on an ::after layer above xterm's positioned layers, which paint over the element's own outline", () => {
    const body = css.match(rule(/\.terminal-host\[data-drop-over\]::after,\s*\.side-pane\[data-drop-over\]::after/))?.[1] ?? "";
    expect(body).toMatch(/content:\s*""/);
    expect(body).toMatch(/position:\s*absolute/);
    expect(body).toMatch(/inset:\s*0/);
    expect(body).toMatch(/outline:\s*2px dashed var\(--accent\)/);
    expect(body).toMatch(/outline-offset:\s*-2px/);
    expect(body).toMatch(/pointer-events:\s*none/);
    expect(Number(body.match(/z-index:\s*(\d+)/)?.[1])).toBeGreaterThan(11);
  });
  it("has no rule for a pane: the window draws one terminal and no .pane", () => {
    expect(css).not.toMatch(/\.pane\b/);
  });
  it("draws no bare [data-drop-over] outline or ::after, which would be hidden under xterm", () => {
    expect(css).not.toMatch(/(?:^|\})\s*\[data-drop-over\]\s*\{/);
    expect(css).not.toMatch(/(?:^|[\s,}])\[data-drop-over\]::after/);
  });
  it("draws the label with ::before", () => {
    expect(css).toMatch(/\[data-drop-over\]::before\s*\{[^}]*content:\s*attr\(data-drop-label\)/);
  });
  it("gives the frame and label a positioned box on the side pane and on a terminal host", () => {
    expect(css).toMatch(/\.side-pane\[data-drop-over\],\s*\.terminal-host\[data-drop-over\]\s*\{[^}]*position:\s*relative/);
  });
  it("uses Highlight in forced colours", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*::after[\s\S]*Highlight/);
  });
});
