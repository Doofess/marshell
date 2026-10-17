import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Drop.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

describe("Drop.css", () => {
  it("outlines the drop target with a dashed accent line", () => {
    expect(css).toMatch(/\[data-drop-over\]\s*\{[^}]*outline:\s*2px dashed var\(--accent\)/);
  });
  it("draws the label with ::before, since a pane's ::after is its focus ring (SplitView.css)", () => {
    expect(css).toMatch(/\[data-drop-over\]::before\s*\{[^}]*content:\s*attr\(data-drop-label\)/);
    expect(css).not.toMatch(/\[data-drop-over\]::after/);
  });
  it("gives the label a positioned box on the preview and on a terminal host outside a pane", () => {
    expect(css).toMatch(/\.preview\[data-drop-over\],\s*\.terminal-host\[data-drop-over\]\s*\{[^}]*position:\s*relative/);
  });
  it("uses Highlight in forced colours", () => {
    expect(css).toMatch(/@media \(forced-colors: active\)[\s\S]*Highlight/);
  });
});
