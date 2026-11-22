import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FilesBusy } from "./FilesBusy";

describe("FilesBusy", () => {
  it("is a 40 px ring hidden from a screen reader, with its line under it", () => {
    const html = renderToStaticMarkup(<FilesBusy text="Searching…" />);
    expect(html).toContain('width="40"');
    expect(html).toContain('height="40"');
    expect(html).toContain('aria-hidden="true"');
    expect(html).toContain('<p class="files-busy__text">Searching…</p>');
  });
  const css = readFileSync(new URL("./FilesBusy.css", import.meta.url), "utf8");
  it("turns only when motion is allowed, and uses system colours in forced colours", () => {
    const at = css.indexOf("animation: files-busy-spin");
    expect(at).toBeGreaterThan(-1);
    const before = css.slice(0, at);
    expect(before).toContain("@media (prefers-reduced-motion: no-preference)");
    expect(before.slice(before.lastIndexOf("@media"))).toContain("@media (prefers-reduced-motion: no-preference)");
    expect(before.slice(before.lastIndexOf("@media"))).toContain(':root:not([data-motion="reduce"]) .files-busy__ring');
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(forced).toContain("CanvasText");
    expect(forced).toContain("GrayText");
    expect(forced).not.toMatch(/var\(--|#[0-9a-f]{3}/i);
  });
});
