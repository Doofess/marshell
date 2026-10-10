import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { AGENT_IDS } from "./agents";

const read = (p: string) => readFileSync(new URL(p, import.meta.url), "utf8");
const colors = read("./agent-colors.css");
const tokens = parseTokens(read("../../styles/tokens.css"), ":root");
const THEMES = ["dark", "light"] as const;
const SURFACES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;

const rules = [...colors.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ selectors: m[1]!.split(",").map((x) => x.trim()), body: m[2]! }));
/** The last rule for a selector that declares `name`, as the cascade would pick it. */
const declared = (selector: string, name: string) => {
  for (const r of [...rules].reverse()) {
    if (!r.selectors.includes(selector)) continue;
    const m = r.body.match(new RegExp(String.raw`${name}:\s*([^;]+);`));
    if (m) return m[1]!.trim();
  }
  return undefined;
};
const block = (selector: string) => rules.filter((r) => r.selectors.includes(selector)).map((r) => r.body).join("\n");
const signalToken = (id: string) => declared(`[data-agent="${id}"]`, "--agent-signal")!.match(/var\((--[\w-]+)\)/)![1]!;
/** The ring and bar colour: the signal colour, unless the vendor names a stronger one. */
const focusToken = (id: string) => declared(`[data-agent="${id}"]`, "--agent-focus")?.match(/var\((--[\w-]+)\)/)?.[1] ?? signalToken(id);
/** The ink on a vendor-coloured button: the shared one, or the vendor's own. */
const inkOf = (id: string, theme: "dark" | "light") => {
  const v = declared(`[data-agent="${id}"]`, "--agent-ink") ?? declared("[data-agent]", "--agent-ink")!;
  return parseTokens(`:root{--x:${v};}`, ":root")["--x"]![theme];
};

describe("the app accent follows the vendor", () => {
  const shared = block("[data-agent]");
  it("takes the signal colour as the accent, anywhere inside a session's element", () => {
    expect(shared).toMatch(/--accent:\s*var\(--agent-signal\)/);
    expect(shared).toMatch(/--accent-ink:\s*var\(--agent-ink\)/);
  });
  it("re-derives the tint and flash from that accent, since a derived token is fixed where it is declared", () => {
    expect(shared).toMatch(/--accent-tint:\s*color-mix\(in oklch, var\(--accent\)/);
    expect(shared).toMatch(/--accent-flash:\s*color-mix\(in oklch, var\(--accent\)/);
  });
  it("names a focus colour the rings and bars use", () => {
    expect(shared).toMatch(/--focus:\s*var\(--agent-focus, var\(--agent-signal\)\)/);
  });
  for (const theme of THEMES)
    for (const id of AGENT_IDS) {
      it(`${id}: text on its button fill holds 4.5:1 (${theme})`, () => {
        expect(contrast(inkOf(id, theme), tokens[signalToken(id)]![theme])).toBeGreaterThanOrEqual(4.5);
      });
      for (const s of SURFACES)
        it(`${id}: its focus ring and active-pane bar hold 3:1 on ${s} (${theme})`, () => {
          expect(contrast(tokens[focusToken(id)]![theme], tokens[s]![theme])).toBeGreaterThanOrEqual(3);
        });
    }
});
