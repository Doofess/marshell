import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..");
const read = (f: string) => readFileSync(join(SRC, f), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (css: string, selector: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].find((m) => m[1]!.split(",").map((s) => s.trim()).includes(selector))?.[2];

/**
 * The fix sheet and the onboarding frame can hold a change of any length (three consent cards, a 40-line diff), and the
 * window is at least 720 by 480. Their body scrolls with a visible themed scrollbar (like the drawer and the page body) and
 * the actions are a last grid row outside it, so Cancel, the fix and "Allow these changes" are always reachable.
 */
const DIALOGS: Array<{ name: string; file: string; root: string; body: string; foot: string }> = [
  { name: "the fix sheet", file: "features/doctor/Doctor.css", root: ".fix-sheet", body: ".fix-sheet__body", foot: ".fix-sheet__foot" },
  { name: "the onboarding frame", file: "features/onboarding/Onboarding.css", root: ".onboarding", body: ".onboarding__body", foot: ".onboarding__foot" },
];

describe("dialog bodies", () => {
  for (const d of DIALOGS) {
    const css = read(d.file);
    it(`${d.name}: only its body scrolls, and the scrollbar stays visible`, () => {
      expect(rule(css, d.body)).toMatch(/overflow-y:\s*auto/);
      expect(rule(css, d.body)).toMatch(/min-block-size:\s*0/);
      expect(css).not.toMatch(/scrollbar-width:\s*none|::-webkit-scrollbar\s*\{\s*display:\s*none/);
      const scrollers = [...css.matchAll(/([^{}]+)\{([^{}]*overflow(?:-x|-y)?:\s*(?:auto|scroll)[^{}]*)\}/g)].map((m) => m[1]!.trim());
      expect(scrollers).toEqual([d.body]);
    });
    it(`${d.name}: stays inside its space, with the actions as the last row under the scrolling body`, () => {
      const root = rule(css, d.root)!;
      expect(root).toMatch(/max-block-size:\s*100%/);
      expect(root).toMatch(/grid-template-rows:[^;]*minmax\(0,\s*1fr\)\s+auto\s*;/);
      expect(css).toContain(d.foot);
    });
  }
  // A grid's auto row grows to its item, so a sheet's `max-block-size: 100%` held only in a row as tall as the window (live check 6a).
  for (const w of [
    { name: "the onboarding frame", file: "styles/app.css", wrapper: ".live-app__onboarding" },
    { name: "the Settings page", file: "features/settings/LiveSettings.css", wrapper: ".live-settings" },
  ]) {
    it(`${w.name}: sits in a row as tall as the window, so its body scrolls`, () => {
      expect(rule(read(w.file), w.wrapper)).toMatch(/grid-template-rows:\s*minmax\(0,\s*1fr\)/);
    });
  }
  it("the scripts trust card: only its list scrolls, capped below the window, with the scrollbar visible", () => {
    const css = read("features/scripts/ScriptsTrust.css");
    expect(rule(css, ".scripts-trust__list")).toMatch(/overflow-y:\s*auto/);
    expect(rule(css, ".scripts-trust__list")).toMatch(/max-block-size:\s*min\(40vh/);
    expect(css).not.toMatch(/scrollbar-width:\s*none|::-webkit-scrollbar\s*\{\s*display:\s*none/);
    const scrollers = [...css.matchAll(/([^{}]+)\{([^{}]*overflow(?:-x|-y)?:\s*(?:auto|scroll)[^{}]*)\}/g)].map((m) => m[1]!.trim());
    expect(scrollers).toEqual([".scripts-trust__list"]);
  });
  it("lists both bodies in the no-scroll allowlist, with a reason", () => {
    const test = readFileSync(join(SRC, "styles/noScrollInMainWindow.test.ts"), "utf8");
    expect(test).toContain("fix-sheet__body");
    expect(test).toContain("onboarding__body");
  });
});
