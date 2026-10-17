import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./SplitView.css", import.meta.url), "utf8");
const rule = (s: string) => {
  const i = css.indexOf(`${s} {`);
  return i < 0 ? "" : css.slice(i, css.indexOf("}", i));
};

describe("SplitView.css", () => {
  it("dims what an inactive pane header holds to 60%, but never the terminal text, a pane that needs you, or the narrow-pane warning", () => {
    expect(rule('.pane[data-active="false"] .pane__header:not([data-needs="true"]) > :not(.chip)')).toContain("opacity: 0.6");
    expect(css).not.toMatch(/\.pane[^{]*\.scripted-terminal[^{]*\{[^}]*opacity/);
  });
  it("rings the focused pane with a 1 px focus inset and a 4 px 14% halo, on an overlay that is invisible on every other pane", () => {
    const base = rule(".pane::after");
    expect(base).toContain('content: ""');
    expect(base).toContain("pointer-events: none");
    expect(base).toContain("border-radius: var(--radius)");
    expect(base).toContain("opacity: 0");
    expect(base).toContain("opacity 120ms ease-out");
    // The ring is drawn on every pane of a split and only its opacity follows focus, so it fades out as well as in.
    const ring = rule(".pane[data-split]::after");
    expect(ring).toContain("inset 0 0 0 1px var(--focus, var(--accent))");
    expect(ring).toContain("inset 0 0 0 4px color-mix(in oklch, var(--focus, var(--accent)) 14%, transparent)");
    const on = rule('.pane[data-active="true"][data-split]::after');
    expect(on).toContain("opacity: 1");
    expect(on).not.toContain("box-shadow");
  });
  it("keeps the ring above the pane's own layers (xterm up to z 11) without letting it past the pane", () => {
    expect(Number(/z-index:\s*(\d+)/.exec(rule(".pane::after"))?.[1])).toBeGreaterThan(11);
    expect(rule(".pane")).toContain("isolation: isolate");
  });
  it("raises the focused header with the fill, with no bar under any header", () => {
    expect(rule('.pane[data-active="true"] .pane__header')).toContain("background-color: var(--fill)");
    expect(css).not.toContain("border-block-end");
    expect(rule(".pane__header")).toContain("transition: background-color 120ms ease-out");
  });
  it("lays the danger and needs tints over the fill instead of replacing it", () => {
    for (const s of ['.pane__header[data-danger]', '.pane__header[data-needs="true"]']) {
      expect(rule(s)).toMatch(/background-image:\s*linear-gradient\(/);
      expect(rule(s)).not.toMatch(/background:/);
    }
  });
  it("turns the crossfade off under reduced motion", () => {
    const m = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
    expect(m).toMatch(/\.pane::after[\s\S]*?transition: none/);
    expect(m).toMatch(/\.pane__header[\s\S]*?transition: none/);
  });
  it("outlines the focused pane in Highlight in forced colours, where shadows vanish", () => {
    const m = css.slice(css.indexOf("@media (forced-colors: active)"));
    expect(m).toMatch(/\.pane\[data-active="true"\]\[data-split\]::after\s*\{[^}]*outline: 2px solid Highlight;[^}]*outline-offset: -2px/);
  });
  it("makes a divider one pixel of hairline with an 8 px hit area that overhangs the panes", () => {
    expect(rule(".split__handle")).toContain("--hit: 8px");
    expect(rule(".split__handle")).toContain("background: var(--hairline)");
    expect(css).toContain(".split__handle::before");
    expect(css).toMatch(/inset-inline:\s*calc\(\(var\(--hit\) - 1px\) \/ -2\)/);
    expect(css).toContain("col-resize");
    expect(css).toContain("row-resize");
  });
  it("keeps the receding header readable: its secondary text and buttons use full-strength ink so 60% opacity still passes AA", () => {
    expect(css).toMatch(/\.pane\[data-active="false"\] \.pane__place,[\s\S]*?\{\s*color: var\(--text-1\)/);
  });
  it("re-inks the session number in the receding header too, so the numeral keeps AA at 60%", () => {
    expect(css).toMatch(/\.pane\[data-active="false"\] \.session-number[\s\S]*?\{\s*color: var\(--text-1\)/);
  });
  it("says in its comment the tint percentage the code really uses", () => {
    const pct = rule('.pane__header[data-needs="true"]').match(/(\d+)%/)?.[1];
    const comment = css.match(/\/\*[^*]*attention tint[^*]*\*\//)?.[0] ?? "";
    expect(pct).toBeDefined();
    expect(comment).toContain(`${pct}%`);
  });
  it("tints a pane that needs you with the vendor colour", () => {
    expect(rule('.pane__header[data-needs="true"]')).toContain("var(--agent-signal");
  });
  it("makes the header buttons 24 px, the minimum target", () => {
    const r = rule(".pane__button");
    expect(r).toContain("inline-size: 24px");
    expect(r).toContain("block-size: 24px");
  });
  it("styles focus and press, hover only for a fine pointer", () => {
    expect(css).toContain(".pane__button:focus-visible");
    expect(css).toContain(".pane__button:active");
    expect(css).toContain(".split__handle:focus-visible");
    expect(css).toMatch(/@media \(hover: hover\) and \(pointer: fine\)/);
  });
  it("never scrolls an empty pane: it clips, because the main window scrolls nowhere but the conversation", () => {
    expect(rule(".pane__empty")).toContain("overflow: clip");
    expect(css).not.toMatch(/overflow(-[xy])?:\s*(auto|scroll)/);
  });
  it("docks each pane's prompt line in a fixed slot at the pane's foot, growing upward over its own terminal", () => {
    const r = rule(".pane__composer");
    expect(r).toContain("position: relative");
    expect(r).toContain("flex: none");
    const inner = rule(".pane__composer > .composer");
    expect(inner).toContain("position: absolute");
    expect(inner).toContain("inset-block-end: 0");
    expect(inner).toContain("inset-inline: 0");
    expect(inner).toMatch(/z-index:\s*2/);
  });
  it("uses tokens and logical properties only", () => {
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(css).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(css).not.toMatch(/\b(left|right|top|bottom):/);
  });
});

describe("SplitView.css: a pane header is a title bar, so the session's name is the last thing to give way", () => {
  it("lets the project shrink long before the name (a much larger shrink factor), and gives the name a floor", () => {
    const place = Number(/flex-shrink:\s*(\d+)/.exec(rule(".pane__place"))?.[1] ?? 1);
    const name = Number(/flex-shrink:\s*(\d+)/.exec(rule(".pane__name"))?.[1] ?? 1);
    expect(place).toBeGreaterThan(name);
    expect(rule(".pane__name")).toMatch(/min-inline-size:\s*(?!0[;\s])[^;]+/);
  });
  it("never lets the number or the status glyph shrink", () => {
    expect(rule(".pane__header > .session-number")).toContain("flex: none");
  });
});

describe("SplitView.css: the restart bar in a pane", () => {
  it("is as tall as the pane header it replaces", () => {
    expect(rule(".pane > .close-bar")).toContain("block-size: 28px");
  });
});
