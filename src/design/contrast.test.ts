import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces, overlay } from "./contrast";

const ACCENTS = ["amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;
const THEMES = ["dark", "light"] as const;
// Read from disk: Vitest hands CSS imports (even ?raw) to tests as empty strings.
const read = (f: string) => readFileSync(new URL(`../styles/${f}`, import.meta.url), "utf8");
const tokensCss = read("tokens.css");
const accentsCss = read("accents.css");
const tokens = parseTokens(tokensCss, ":root");
const accent = (name: string) => parseTokens(accentsCss, `[data-accent="${name}"]`);

describe("text on surfaces", () => {
  for (const theme of THEMES)
    for (const t of ["--text-1", "--text-2", "--text-3"])
      it(`${t} is at least 4.5:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[t]![theme], tokens, theme)).toBeGreaterThanOrEqual(4.5);
      });
});

describe("status and brand colours on surfaces", () => {
  const names = ["--error", "--ok", "--caution", "--brand-claude", "--brand-claude-mark", "--brand-codex", "--brand-openai", "--brand-cyan", "--brand-gemini", "--brand-deepseek", "--brand-violet", "--brand-generic"];
  for (const theme of THEMES)
    for (const n of names)
      it(`${n} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[n]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
});

describe("accents", () => {
  it("defines all eight", () => {
    for (const a of ACCENTS) expect(accent(a)["--accent"], a).toBeDefined();
  });
  for (const theme of THEMES)
    for (const a of ACCENTS) {
      it(`${a} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(accent(a)["--accent"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
      it(`${a} ink is at least 4.5:1 on the accent (${theme})`, () => {
        const v = accent(a);
        expect(contrast(v["--accent-ink"]![theme], v["--accent"]![theme])).toBeGreaterThanOrEqual(4.5);
      });
    }
});

describe("distinctness", () => {
  for (const theme of THEMES)
    for (const a of ACCENTS)
      for (const s of ["--error", "--ok", "--caution"])
        it(`${a} is ΔE ≥ 15 from ${s} (${theme})`, () => {
          expect(deltaE(accent(a)["--accent"]![theme], tokens[s]![theme])).toBeGreaterThanOrEqual(15);
        });
  for (const theme of THEMES)
    it(`default amber is ΔE ≥ 18 from Claude orange (${theme})`, () => {
      expect(deltaE(accent("amber")["--accent"]![theme], tokens["--brand-claude"]![theme])).toBeGreaterThanOrEqual(18);
    });
  it("the :root default is amber", () => {
    expect(parseTokens(accentsCss, ":root")["--accent"]).toEqual(accent("amber")["--accent"]);
  });
});

describe("composited and derived colours (final review)", () => {
  for (const theme of THEMES)
    it(`--ok-dim (the seen check) is at least 3:1 on every surface (${theme})`, () => {
      expect(minOnSurfaces(tokens["--ok-dim"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
    });
  const brands = ["--brand-claude", "--brand-codex", "--brand-openai", "--brand-cyan", "--brand-gemini", "--brand-deepseek", "--brand-violet", "--brand-generic"];
  for (const theme of THEMES)
    for (const s of ["--error", "--ok", "--caution"])
      for (const b of brands)
        it(`${s} is ΔE ≥ 15 from ${b} (${theme})`, () => {
          expect(deltaE(tokens[s]![theme], tokens[b]![theme])).toBeGreaterThanOrEqual(15);
        });
});

describe("text on the blocked-row tints", () => {
  for (const theme of THEMES)
    for (const a of ACCENTS)
      for (const t of ["--text-1", "--text-2"])
        for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
          it(`${t} is at least 4.5:1 on the ${a} needs-you tint (${theme}, ${surface})`, () => {
            const bg = overlay(accent(a)["--accent"]![theme], tokens[surface]![theme], 0.12);
            expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
          });
  for (const theme of THEMES)
    for (const t of ["--text-1", "--text-2"])
      for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
        it(`${t} is at least 4.5:1 on the error tint (${theme}, ${surface})`, () => {
          const bg = overlay(tokens["--error"]![theme], tokens[surface]![theme], 0.1);
          expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
        });
});

describe("OpenAI's light yellow and Antigravity's cyan (agent colours)", () => {
  for (const theme of THEMES)
    for (const [name, other] of [
      ["--brand-openai", "--brand-codex"],
      ["--brand-cyan", "--brand-gemini"],
      ["--brand-cyan", "--brand-deepseek"],
    ] as const)
      it(`${name} is ΔE ≥ 15 from ${other} (${theme})`, () => {
        expect(deltaE(tokens[name]![theme], tokens[other]![theme])).toBeGreaterThanOrEqual(15);
      });
  for (const theme of THEMES)
    for (const name of ["--brand-openai", "--brand-cyan"])
      for (const t of ["--text-1", "--text-2"])
        for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
          it(`${t} is at least 4.5:1 on the ${name} blocked-row tint (${theme}, ${surface})`, () => {
            const bg = overlay(tokens[name]![theme], tokens[surface]![theme], 0.12);
            expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
          });
  for (const theme of THEMES)
    for (const name of ["--brand-openai", "--brand-cyan"])
      it(`the mark on a ${name} badge (the needs-you ink) is at least 3:1 (${theme})`, () => {
        const ink = theme === "dark" ? "oklch(0% 0 none)" : "oklch(100% 0 none)";
        expect(contrast(ink, tokens[name]![theme])).toBeGreaterThanOrEqual(3);
      });
});

describe("text on hover and selected rows (batch 1 deferred minor)", () => {
  for (const theme of THEMES)
    for (const [state, alpha] of [["hover", 0.04], ["selected", 0.07]] as const)
      for (const t of ["--text-2", "--text-3"])
        for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
          it(`${t} is at least 4.5:1 on ${surface} under ${state} (${theme})`, () => {
            const bg = overlay(tokens["--text-1"]![theme], tokens[surface]![theme], alpha);
            expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
          });
});
