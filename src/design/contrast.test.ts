import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseTokens } from "./cssTokens";
import { contrast, deltaE, minOnSurfaces, overlay } from "./contrast";

const THEMES = ["dark", "light"] as const;
const SURFACE_NAMES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;
// Read from disk: Vitest hands CSS imports (even ?raw) to tests as empty strings.
const read = (f: string) => readFileSync(new URL(`../styles/${f}`, import.meta.url), "utf8");
const tokensCss = read("tokens.css");
const accentsCss = read("accents.css");
const tokens = parseTokens(tokensCss, ":root");
const appAccent = parseTokens(accentsCss, ":root");

describe("text on surfaces", () => {
  for (const theme of THEMES)
    for (const t of ["--text-1", "--text-2", "--text-3"])
      it(`${t} is at least 4.5:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[t]![theme], tokens, theme)).toBeGreaterThanOrEqual(4.5);
      });
});

describe("code colours", () => {
  const codeTokens = Object.keys(tokens).filter((n) => n.startsWith("--code-token-"));
  it("every_code_token_is_at_least_4_5_to_1_on_every_surface", () => {
    expect(codeTokens).toHaveLength(12);
    for (const theme of THEMES)
      for (const n of codeTokens) expect(minOnSurfaces(tokens[n]![theme], tokens, theme), `${n} (${theme})`).toBeGreaterThanOrEqual(4.5);
  });
  it("the_four_main_code_hues_differ", () => {
    for (const theme of THEMES) {
      const names = ["--code-token-keyword", "--code-token-string", "--code-token-constant", "--code-token-function"];
      for (let i = 0; i < names.length; i++)
        for (let j = i + 1; j < names.length; j++)
          expect(deltaE(tokens[names[i]!]![theme], tokens[names[j]!]![theme]), `${names[i]} and ${names[j]} (${theme})`).toBeGreaterThanOrEqual(15);
    }
  });
});

describe("conflict tints in the editor", () => {
  const css = readFileSync(new URL("../features/editor/Editor.css", import.meta.url), "utf8");
  // The tints are drawn translucent over the paper, so the selection layer behind the line still shows through.
  const alphaOf = (kind: string) => {
    const m = new RegExp(`editor__conflict--${kind}\\s*\\{[^}]*color-mix\\(in oklch, var\\(--code-conflict-\\w+\\) (\\d+)%, transparent\\)`).exec(css);
    return m ? Number(m[1]) / 100 : undefined;
  };
  const tints = [
    ["--code-conflict-mine", "mine"],
    ["--code-conflict-disk", "disk"],
    ["--code-conflict-mark", "mark"],
  ] as const;
  it("defines the three tints", () => {
    for (const [name] of tints) expect(tokens[name], name).toBeDefined();
  });
  for (const theme of THEMES)
    for (const [name, kind] of tints)
      it(`every code colour is at least 4.5:1 on the ${kind} tint over the paper (${theme})`, () => {
        const a = alphaOf(kind);
        expect(a, `alpha of ${kind} in Editor.css`).toBeDefined();
        const bg = overlay(tokens[name]![theme], tokens["--bg-base"]![theme], a!);
        const code = [...Object.keys(tokens).filter((n) => n.startsWith("--code-token-")), "--code-foreground"];
        for (const n of code) {
          const colour = n === "--code-foreground" ? tokens["--text-1"]![theme] : tokens[n]![theme];
          expect(contrast(colour, bg), `${n} on ${kind} (${theme})`).toBeGreaterThanOrEqual(4.5);
        }
      });
});

describe("paper tints in a PDF", () => {
  // A page is white paper in both themes; the tints are one value, drawn opaque with mix-blend-mode: multiply.
  for (const name of ["--paper-match", "--paper-match-current"])
    it(`black print on ${name} over white paper is at least 7:1`, () => {
      expect(tokens[name], name).toBeDefined();
      const bg = overlay(tokens[name]!.light, "#ffffff", 1);
      expect(contrast("#000000", bg), name).toBeGreaterThanOrEqual(7);
    });
});

const STATUS_AND_BRAND = ["--error", "--ok", "--caution", "--brand-claude", "--brand-claude-mark", "--brand-codex", "--brand-openai", "--brand-cyan", "--brand-deepseek", "--brand-violet", "--brand-generic", "--tool-shell", "--tool-dev", "--tool-remote"];

describe("status and brand colours on surfaces", () => {
  const names = STATUS_AND_BRAND;
  for (const theme of THEMES)
    for (const n of names)
      it(`${n} is at least 3:1 on every surface (${theme})`, () => {
        expect(minOnSurfaces(tokens[n]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
      });
});

describe("the app accent", () => {
  it("is sand, on :root alone: no preset blocks and no data-accent", () => {
    expect(appAccent["--accent"]).toEqual({ light: "oklch(42% 0.028 70)", dark: "oklch(80% 0.045 75)" });
    expect(appAccent["--accent-ink"]).toEqual({ light: "oklch(100% 0 none)", dark: "oklch(0% 0 none)" });
    expect(accentsCss).not.toContain("data-accent");
  });
  for (const theme of THEMES) {
    it(`is at least 3:1 on every surface (${theme})`, () => {
      expect(minOnSurfaces(appAccent["--accent"]![theme], tokens, theme)).toBeGreaterThanOrEqual(3);
    });
    it(`its ink is at least 4.5:1 on it (${theme})`, () => {
      expect(contrast(appAccent["--accent-ink"]![theme], appAccent["--accent"]![theme])).toBeGreaterThanOrEqual(4.5);
    });
  }
});

describe("distinctness", () => {
  for (const theme of THEMES)
    for (const s of ["--error", "--ok", "--caution"])
      it(`the app accent is ΔE ≥ 15 from ${s} (${theme})`, () => {
        expect(deltaE(appAccent["--accent"]![theme], tokens[s]![theme])).toBeGreaterThanOrEqual(15);
      });
  // The window's own colour (no session in play) must not read as a vendor's: Codex is a neutral grey, OpenAI a pale yellow.
  for (const theme of THEMES)
    for (const b of ["--brand-codex", "--brand-openai", "--brand-claude"])
      it(`the app accent is ΔE ≥ 12 from ${b} (${theme})`, () => {
        expect(deltaE(appAccent["--accent"]![theme], tokens[b]![theme])).toBeGreaterThanOrEqual(12);
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
    for (const t of ["--text-1", "--text-2"])
      for (const surface of ["--bg-base", "--bg-raised", "--bg-overlay"])
        it(`${t} is at least 4.5:1 on the app accent's needs-you tint (${theme}, ${surface})`, () => {
          const bg = overlay(appAccent["--accent"]![theme], tokens[surface]![theme], 0.12);
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

  // High contrast draws other surfaces (true black, pure white), so what must hold on a surface must hold on these too.
  it("has surfaces of its own", () => {
    for (const theme of THEMES) expect(hc["--bg-base"]![theme]).not.toBe(tokens["--bg-base"]![theme]);
  });
  it("every code token is at least 4.5:1 on every high contrast surface", () => {
    const codeTokens = Object.keys(hc).filter((n) => n.startsWith("--code-token-"));
    expect(codeTokens).toHaveLength(12);
    for (const theme of THEMES) for (const n of codeTokens) expect(minOnSurfaces(hc[n]![theme], hc, theme), `${n} (${theme})`).toBeGreaterThanOrEqual(4.5);
  });
  it("every status and brand colour is at least 3:1 on every high contrast surface", () => {
    for (const theme of THEMES) for (const n of STATUS_AND_BRAND) expect(minOnSurfaces(hc[n]![theme], hc, theme), `${n} (${theme})`).toBeGreaterThanOrEqual(3);
  });
  it("the app accent is at least 3:1 on every high contrast surface", () => {
    for (const theme of THEMES) expect(minOnSurfaces(appAccent["--accent"]![theme], hc, theme), theme).toBeGreaterThanOrEqual(3);
  });

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
