import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces, overlay } from "./contrast";

const ACCENTS = ["sand", "amber", "blue", "indigo", "violet", "magenta", "cyan", "teal", "slate"] as const;
const THEMES = ["dark", "light"] as const;
const SURFACE_NAMES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;
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
  const names = ["--error", "--ok", "--caution", "--brand-claude", "--brand-claude-mark", "--brand-codex", "--brand-openai", "--brand-cyan", "--brand-deepseek", "--brand-violet", "--brand-generic", "--tool-shell", "--tool-dev", "--tool-remote"];
  for (const theme of THEMES)
    for (const n of names)
      it(`${n} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[n]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
});

describe("accents", () => {
  it("defines all nine", () => {
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
  // The window's own colour (no session in play) must not read as a vendor's: Codex is a neutral grey, OpenAI a pale yellow.
  for (const theme of THEMES)
    for (const b of ["--brand-codex", "--brand-openai", "--brand-claude"])
      it(`default sand is ΔE ≥ 12 from ${b} (${theme})`, () => {
        expect(deltaE(accent("sand")["--accent"]![theme], tokens[b]![theme])).toBeGreaterThanOrEqual(12);
      });
  it("the :root default is sand: dark grey in light mode, beige in dark", () => {
    expect(parseTokens(accentsCss, ":root")["--accent"]).toEqual(accent("sand")["--accent"]);
  });
});

describe("composited and derived colours (final review)", () => {
  for (const theme of THEMES)
    it(`--ok-dim (the seen check) is at least 3:1 on every surface (${theme})`, () => {
      expect(minOnSurfaces(tokens["--ok-dim"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
    });
  const brands = ["--brand-claude", "--brand-codex", "--brand-openai", "--brand-cyan", "--brand-deepseek", "--brand-violet", "--brand-generic", "--tool-shell", "--tool-dev", "--tool-remote"];
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

describe("high contrast", () => {
  const appearanceCss = read("appearance.css").replace(/\/\*[\s\S]*?\*\//g, "");
  const hc = { ...tokens, ...parseTokens(appearanceCss, ':root[data-contrast="more"]') };
  // The overlays mix --text-1 into transparent, so their strength is the percentage in the block.
  const block = /:root\[data-contrast="more"\]\s*\{([^}]*)\}/.exec(appearanceCss)?.[1] ?? "";
  const alpha = (name: string) => {
    const m = new RegExp(`${name}:\\s*color-mix\\(in oklch, var\\(--text-1\\) (\\d+)%, transparent\\)`).exec(block);
    return m ? Number(m[1]) / 100 : undefined;
  };
  const OVERLAYS = ["--hairline", "--fill", "--fill-strong", "--hover", "--selected"];
  const TEXT = ["--text-1", "--text-2", "--text-3"];

  for (const theme of THEMES)
    for (const t of TEXT)
      it(`high contrast text reaches 7:1: ${t} on every surface (${theme})`, () => {
        expect(minOnSurfaces(hc[t]![theme], hc, theme)).toBeGreaterThanOrEqual(7);
      });
  for (const name of OVERLAYS)
    it(`${name} is at least 20% in high contrast`, () => {
      expect(alpha(name)).toBeGreaterThanOrEqual(0.2);
    });
  for (const theme of THEMES)
    for (const surface of SURFACE_NAMES)
      it(`the hairline divider is at least 3:1 on ${surface} in high contrast (${theme})`, () => {
        const line = overlay(hc["--text-1"]![theme], hc[surface]![theme], alpha("--hairline")!);
        expect(contrast(line, hc[surface]![theme])).toBeGreaterThanOrEqual(3);
      });
  for (const theme of THEMES)
    for (const state of ["--hover", "--selected", "--fill"])
      for (const t of TEXT)
        it(`${t} stays at least 7:1 under ${state} in high contrast (${theme})`, () => {
          const bg = overlay(hc["--text-1"]![theme], hc["--bg-overlay"]![theme], alpha(state)!);
          expect(contrast(hc[t]![theme], bg)).toBeGreaterThanOrEqual(7);
        });
});
