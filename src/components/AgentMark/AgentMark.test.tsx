import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AGENT_IDS, AGENT_NAMES, MARK_PATHS } from "./agents";
import { AgentMark } from "./AgentMark";

// Read from disk: Vitest hands CSS imports to tests as empty strings.
const css = readFileSync(new URL("./agent-colors.css", import.meta.url), "utf8");

describe("AgentMark", () => {
  it("knows the CLIs people run, with a generic fallback", () => {
    for (const id of ["claude", "codex", "gemini", "copilot", "cursor", "generic"]) expect(AGENT_IDS).toContain(id);
  });
  for (const id of AGENT_IDS) {
    it(`draws ${id} with a name and an outline`, () => {
      expect(AGENT_NAMES[id].length).toBeGreaterThan(0);
      expect(MARK_PATHS[id].length).toBeGreaterThan(20);
      const html = renderToStaticMarkup(<AgentMark agent={id} />);
      expect(html).toContain(`data-agent="${id}"`);
      expect(html).toContain('viewBox="0 0 24 24"');
      expect(html).toContain(`aria-label="${AGENT_NAMES[id]}"`);
    });
    it(`colours ${id} from a brand token`, () => {
      expect(css).toMatch(new RegExp(String.raw`\[data-agent="${id}"\][^{]*\{[^}]*--agent:\s*var\(--brand-`));
    });
  }
  it("fills the mark with the agent colour and never a literal one", () => {
    const html = renderToStaticMarkup(<AgentMark agent="claude" />);
    expect(html).toContain('fill="currentColor"');
    expect(html).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
  it("falls back to the generic mark for an agent it does not know", () => {
    const html = renderToStaticMarkup(<AgentMark agent={"made-up" as never} />);
    expect(html).toContain('data-agent="generic"');
  });
});
