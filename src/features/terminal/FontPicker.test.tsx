import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { chooseOther, commitText, FontPicker, fieldKey, SAMPLE_LINE } from "./FontPicker";
import { fontStack } from "./terminalFont";

const noop = () => {};
const html = (family: string, wtFamily?: string) => renderToStaticMarkup(<FontPicker family={family} wtFamily={wtFamily} onPick={noop} />);
const LABELS = ["Match my terminal", "JetBrains Mono", "Cascadia Mono", "Atkinson Hyperlegible Mono", "Another font…"];
const labels = (m: string) => [...m.matchAll(/<label[^>]*class="font-picker__option[^>]*>.*?<\/label>/gs)].map((x) => x[0]);
const esc = (s: string) => s.replace(/"/g, "&quot;");

describe("FontPicker markup", () => {
  it("is a fieldset named Terminal font with five radios in order", () => {
    const m = html("");
    expect(m).toMatch(/<fieldset[^>]*>\s*<legend[^>]*>Terminal font<\/legend>/);
    expect((m.match(/<input[^>]*type="radio"/g) ?? []).length).toBe(5);
    const names = labels(m).map((l) => LABELS.find((n) => l.includes(`>${n}<`)));
    expect(names).toEqual(LABELS);
  });

  it("checks the choice the saved family means", () => {
    const checked = (m: string) => labels(m).findIndex((l) => l.includes('checked=""'));
    expect(checked(html(""))).toBe(0);
    expect(checked(html("JETBRAINS mono "))).toBe(1);
    expect(checked(html("cascadia mono"))).toBe(2);
    expect(checked(html("Atkinson Hyperlegible Mono"))).toBe(3);
    expect(checked(html("Fira Code"))).toBe(4);
    expect(labels(html("Cascadia Mono"))[2]).toContain('data-selected="true"');
  });

  it("draws each label in its own font, Match my terminal in the one it resolves to", () => {
    const m = labels(html("", "Hack NF"));
    expect(m[0]).toContain(esc(`font-family:${fontStack("Hack NF", "Hack NF")}`));
    expect(m[1]).toContain(esc(`font-family:${fontStack("JetBrains Mono", "Hack NF")}`));
    expect(m[2]).toContain(esc(`font-family:${fontStack("Cascadia Mono", "Hack NF")}`));
    expect(labels(html(""))[0]).toContain(esc(`font-family:${fontStack("JetBrains Mono")}`));
  });

  it("shows the sample line on every label", () => {
    expect(SAMPLE_LINE).toBe("0O 1lI {} => ~/src $ git status");
    for (const l of labels(html(""))) expect(l).toContain(SAMPLE_LINE.replace(">", "&gt;"));
  });

  it("shows the text field only for Another font", () => {
    expect(html("")).not.toContain("Font family");
    expect(html("Cascadia Mono")).not.toContain("Font family");
    const m = html("Fira Code");
    expect(m).toMatch(/<label[^>]*>Font family<\/label>/);
    expect(m).toContain('value="Fira Code"');
  });
});

describe("FontPicker behaviour", () => {
  it("picking Another font shows the field and saves nothing", () => {
    expect(chooseOther({ other: false, text: "" }, "")).toEqual({ other: true, text: "" });
    expect(chooseOther({ other: false, text: "x" }, "Fira Code")).toEqual({ other: true, text: "Fira Code" });
  });
  it("committing a name picks it", () => {
    expect(commitText({ other: true, text: "  Fira Code " }, "")).toEqual({ state: { other: false, text: "" }, pick: "Fira Code" });
  });
  it("committing a bundled font's name, written another way, picks its canonical name", () => {
    expect(commitText({ other: true, text: "cascadia mono " }, "")).toEqual({ state: { other: false, text: "" }, pick: "Cascadia Mono" });
  });
  it("committing an empty field picks nothing and goes back to the choice shown before", () => {
    expect(commitText({ other: true, text: "  " }, "Cascadia Mono")).toEqual({ state: { other: false, text: "" }, pick: null });
    expect(commitText({ other: true, text: "" }, "Fira Code")).toEqual({ state: { other: false, text: "Fira Code" }, pick: null });
  });
  it("Esc reverts the text and is handled", () => {
    expect(fieldKey("Escape", "Fira", "Fira Code")).toEqual({ next: "Fira Code", prevent: true });
    expect(fieldKey("a", "Fira", "Fira Code")).toEqual({ next: "Fira", prevent: false });
  });
});

describe("FontPicker.css", () => {
  const css = readFileSync(new URL("./FontPicker.css", import.meta.url), "utf8");
  it("sets only system colours in forced colours", () => {
    const block = css.match(/@media \(forced-colors: active\) \{((?:[^{}]|\{[^{}]*\})*)\}/)?.[1] ?? "";
    for (const m of block.matchAll(/(?:^|[\s;{])(?:color|background(?:-color)?)\s*:\s*([^;}]+)/g)) expect(m[1]).toMatch(/^\s*(CanvasText|Canvas|Highlight|HighlightText|GrayText|LinkText|ButtonText|ButtonFace|transparent)\s*$/);
  });
});
