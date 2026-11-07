import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./Launcher.css", import.meta.url), "utf8");
const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (s: string) => {
  const i = bare.indexOf(`${s} {`);
  return i < 0 ? "" : bare.slice(i, bare.indexOf("}", i));
};
/** Every `selectors { body }` pair, innermost blocks included. */
const pairs = (text: string) => [...text.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sels: m[1]!.split(",").map((x) => x.trim()), body: m[2]! }));
const FORCED = "@media (forced-colors: active)";
const shipped = bare.slice(0, bare.indexOf(FORCED));
const forced = bare.slice(bare.indexOf(FORCED));
/** Decision 9's list: the prompt line, the Name, Task name and folder fields, the chips, Back and Start. */
const FOCUSED = [".launcher__line:focus-within", ".launcher__name-input:focus-visible", ".launcher__task-input:focus-visible", ".launcher__input:focus-visible", ".launcher__chip:focus-visible", ".launcher__back:focus-visible", ".launcher__start:focus-visible"];
/** The rules for a focused launcher element itself, not for a part inside one. The prompt is left out: its line shows its focus. */
const focusRules = pairs(shipped).flatMap((r) =>
  r.sels.filter((x) => /^\.launcher__[\w-]+(?:\[[^\]]*\])*:focus-(?:visible|within)$/.test(x) && x !== ".launcher__prompt:focus-visible").map((sel) => ({ sel, body: r.body })),
);
const backgroundOf = (sel: string, text = shipped) => pairs(text).find((r) => r.sels.includes(sel) && /background:/.test(r.body))?.body.match(/background:\s*([^;]+)/)?.[1]?.trim();

describe("Launcher.css", () => {
  it("is a 560 px sheet 20% from the top", () => {
    expect(rule(".launcher__sheet")).toContain("inline-size: min(560px");
    expect(rule(".launcher__sheet")).toContain("inset-block-start: 20%");
  });
  it("styles focus and press on chips, programs and options", () => {
    expect(bare).toContain(".launcher__chip:focus-visible");
    expect(bare).toContain(".launcher__chip:active");
    expect(bare).toContain(".launcher__opt:active");
    expect(bare).toContain(".launcher__program:active");
    expect(bare).not.toMatch(/outline:\s*none/);
  });
  it("only styles hover where the device hovers", () => {
    expect(bare.slice(0, bare.indexOf("@media (hover: hover) and (pointer: fine)"))).not.toContain(":hover");
  });
  it("makes the matched letters stand out without relying on colour alone", () => {
    expect(rule(".launcher__opt mark")).toContain("font-weight");
    expect(rule(".launcher__opt mark")).toContain("text-decoration");
  });
  it("marks the active option with a shape as well as a fill: a bar down the side of a row, a line under a program", () => {
    expect(rule(".launcher__opt[data-active]")).toContain("border-inline-start");
    expect(rule(".launcher__program[data-active]")).toContain("box-shadow: inset 0 -2px");
  });
  it("uses tokens and logical properties only", () => {
    expect(bare).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    expect(bare).not.toMatch(/\b(margin|padding|border)-(left|right|top|bottom)\b/);
    expect(bare).not.toMatch(/\b(left|right|top|bottom):/);
  });
  it("tells surfaces apart by fill, with no stroke around a chip, key, list or the sheet", () => {
    const shipped = bare.replace(/@media \(forced-colors: active\) \{[\s\S]*$/, "");
    expect(shipped).not.toMatch(/(?:^|[\s;{])border:\s*(?!0\b)[\d.]+px\s+solid/m);
    expect(shipped).not.toMatch(/(?:^|[\s;{])border-color:/m);
  });
  it("sets the prompt line as a flat bar on the terminal's surface, a hairline on top that focus leaves alone, mono type", () => {
    const line = rule(".launcher__line");
    expect(line).toContain("border-block-start: 2px solid var(--hairline)");
    expect(line).not.toContain("border-radius");
    expect(line).not.toContain("box-shadow");
    expect(bare).not.toMatch(/\.launcher__line:focus-within\s*\{[^}]*border/);
    expect(rule(".launcher__prompt")).toContain("font-family: var(--font-mono)");
    expect(rule(".launcher__glyph")).toContain("color: var(--accent)");
  });
  it("focus_in_the_launcher_is_fill_and_tint_not_a_ring", () => {
    for (const sel of FOCUSED) expect(focusRules.map((r) => r.sel), sel).toContain(sel);
    for (const { sel, body } of focusRules) {
      expect(body, sel).toMatch(/background:/);
      expect(body, sel).not.toMatch(/border-block-start-color:|box-shadow:/);
      // The base ring is hidden, never removed: forced colors repaints a transparent outline, and outline: none would lose it.
      for (const m of body.matchAll(/outline(?:-color)?:\s*([^;]+)/g)) expect(m[1]!.trim(), sel).toMatch(/transparent$/);
      if (sel.endsWith(":focus-visible")) expect(body, sel).toMatch(/outline-color:\s*transparent/);
    }
  });
  it("gives each focusable thing a fill that is neither its rest fill nor its hover fill", () => {
    const hover = bare.slice(bare.indexOf("@media (hover: hover) and (pointer: fine)"), bare.indexOf(FORCED));
    for (const base of [".launcher__chip", ".launcher__back", ".launcher__start", ".launcher__input", ".launcher__task-input", ".launcher__name-input"]) {
      const focus = backgroundOf(`${base}:focus-visible`);
      expect(focus, base).toBeDefined();
      expect(focus, `${base} at rest`).not.toBe(backgroundOf(base));
      const escaped = base.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      const over = hover.match(new RegExp(`${escaped}:hover[^{]*\\{[^}]*background:\\s*([^;]+)`))?.[1]?.trim();
      if (over !== undefined) expect(focus, `${base} on hover`).not.toBe(over);
    }
    expect(backgroundOf(".launcher__line:focus-within")).not.toBe(backgroundOf(".launcher__line"));
  });
  it("puts an accent caret in every launcher text field", () => {
    for (const sel of [".launcher__prompt", ".launcher__input", ".launcher__task-input", ".launcher__name-input"])
      expect(pairs(shipped).some((r) => r.sels.includes(sel) && /caret-color:\s*var\(--accent\)/.test(r.body)), sel).toBe(true);
  });
  it("keeps what sits in a focused chip visible on its accent fill: marks, the caution signs and the check box take the chip's ink", () => {
    expect(shipped).toMatch(/\.launcher__chip:focus-visible :is\([^)]*\.launcher__mark[^)]*\.agent-mark[^)]*\.launcher__risk[^)]*\.launcher__shield[^)]*\)\s*\{[^}]*color:\s*currentColor/);
    expect(shipped).toMatch(/\.launcher__chip:focus-visible \.launcher__box\[data-checked\]\s*\{[^}]*background:\s*var\(--accent-ink\)/);
    expect(forced).toMatch(/\.launcher__chip:focus-visible \.launcher__box\[data-checked\]\s*\{[^}]*background:\s*Highlight/);
  });
  it("gives every focusable control in Launcher.tsx a focus rule here, so the base ring never shows in the launcher", () => {
    const tsx = readFileSync(new URL("./Launcher.tsx", import.meta.url), "utf8");
    const classes = new Set([...tsx.matchAll(/<(?:button|input|textarea)\b[^>]*?className=(?:"|\{[^}]*?")(launcher__[\w-]+)/g)].map((m) => `.${m[1]}`));
    expect(classes.size).toBeGreaterThanOrEqual(7);
    const hidden = pairs(shipped).flatMap((r) => (/outline-color:\s*transparent/.test(r.body) ? r.sels : []));
    for (const c of classes) expect(hidden, c).toContain(`${c}:focus-visible`);
  });
  it("shows the prompt's focus on its line alone: its own ring is hidden, and in forced colors drawn in Canvas inside the line's Highlight", () => {
    expect(rule(".launcher__prompt:focus-visible")).toMatch(/outline-color:\s*transparent/);
    expect(forced).toMatch(/\.launcher__prompt:focus-visible\s*\{[^}]*outline-color:\s*Canvas/);
  });
  it("forced_colors_draws_focus_with_highlight", () => {
    const rules = pairs(forced);
    for (const { sel } of focusRules) expect(rules.some((r) => r.sels.includes(sel) && /outline:\s*2px solid Highlight/.test(r.body)), sel).toBe(true);
  });
  it("shows the Name's default in a token colour at full opacity, as the prompt line's placeholder", () => {
    expect(rule(".launcher__name-input::placeholder")).toContain("color: var(--text-2)");
    expect(rule(".launcher__name-input::placeholder")).toContain("opacity: 1");
  });
  it("keeps text on a fill at AA: the Enter key on the Start button has no chip behind it, and the key name is not dimmed", () => {
    expect(rule(".launcher__start kbd")).toContain("background: none");
    expect(rule(".launcher__start kbd")).not.toContain("color-mix");
  });
  it("lets the folder list scroll with no scrollbar drawn, and fades its edges", () => {
    const list = rule(".launcher__list");
    expect(list).toMatch(/overflow-y:\s*auto/);
    expect(list).toMatch(/scrollbar-width:\s*none/);
    expect(list).toMatch(/overscroll-behavior:\s*contain/);
    expect(bare).toMatch(/\.launcher__list::-webkit-scrollbar\s*\{[^}]*display:\s*none/);
  });
  it("keeps an edge for every filled control in Windows high contrast, and draws the active option with Highlight", () => {
    const forced = css.slice(css.indexOf("@media (forced-colors: active)"));
    for (const sel of [".launcher__sheet", ".launcher__chip", ".launcher__input", ".launcher__start", ".launcher__program"]) expect(forced, sel).toContain(sel);
    expect(forced).toMatch(/outline:\s*1px solid CanvasText/);
    expect(forced).toMatch(/\[data-active\][^}]*\{[^}]*outline:\s*2px solid Highlight/);
  });
  it("reads the vendor's accent from the sheet, so Start and the line follow the agent", () => {
    expect(rule(".launcher__start")).toContain("background: var(--accent)");
    expect(rule(".launcher__start")).toContain("color: var(--accent-ink)");
  });
});
