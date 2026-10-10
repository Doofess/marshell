import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast, overlay } from "../../design/contrast";
import { parseTokens } from "../../design/cssTokens";
import { AGENT_IDS } from "./agents";

const read = (p: string) => readFileSync(new URL(p, import.meta.url), "utf8");
const colors = read("./agent-colors.css");
const tokens = parseTokens(read("../../styles/tokens.css"), ":root");
const THEMES = ["dark", "light"] as const;
const SURFACES = ["--bg-base", "--bg-raised", "--bg-overlay"] as const;

/** The token each vendor's signal colour points at, read from agent-colors.css. */
function signalToken(id: string): string {
  const m = colors.match(new RegExp(String.raw`\[data-agent="${id}"\][^{]*\{[^}]*--agent-signal:\s*var\((--[\w-]+)\)`));
  if (!m) throw new Error(`no --agent-signal for ${id}`);
  return m[1]!;
}
// OpenAI's bright yellow has its own dark ink in both themes (agent-colors.css).
const ink = (theme: "dark" | "light", id?: string) => (id === "codex" ? "#1c1c1c" : theme === "dark" ? "#000000" : "#ffffff");

describe("vendor signal colour (needs-you and error accents)", () => {
  it("is defined for every vendor, from a brand token", () => {
    for (const id of AGENT_IDS) expect(tokens[signalToken(id)], id).toBeDefined();
  });
  it("defines one ink for text and marks on the signal colour", () => {
    expect(colors).toMatch(/--agent-ink:\s*light-dark\(oklch\(100% 0 none\), oklch\(0% 0 none\)\)/);
  });
  for (const theme of THEMES)
    for (const id of AGENT_IDS) {
      it(`the mark on the ${id} badge holds 3:1 (${theme})`, () => {
        expect(contrast(ink(theme, id), tokens[signalToken(id)]![theme])).toBeGreaterThanOrEqual(3);
      });
      for (const t of ["--text-1", "--text-2"])
        for (const s of SURFACES)
          it(`${t} holds 4.5:1 on the ${id} row tint over ${s} (${theme})`, () => {
            const bg = overlay(tokens[signalToken(id)]![theme], tokens[s]![theme], 0.12);
            expect(contrast(tokens[t]![theme], bg)).toBeGreaterThanOrEqual(4.5);
          });
    }
});
