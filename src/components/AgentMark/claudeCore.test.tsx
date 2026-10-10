import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AgentMark, CLAUDE_CORE_RADIUS } from "./AgentMark";

describe("a working Claude mark keeps its centre still while the rays move", () => {
  const h = renderToStaticMarkup(<AgentMark agent="claude" working />);
  it("draws a clipped copy of the centre on top of the animated body", () => {
    expect(h).toContain("agent-mark__body");
    expect(h).toContain("agent-mark__core");
    expect(h).toMatch(new RegExp(`<circle[^>]*r="${CLAUDE_CORE_RADIUS}"`));
    expect(h.indexOf("agent-mark__core")).toBeGreaterThan(h.indexOf("agent-mark__body"));
  });
  it("hides the centre copy from assistive technology", () => {
    expect(h).toMatch(/class="agent-mark__core"[^>]*aria-hidden="true"/);
  });
  it("gives every instance its own clip id", () => {
    const two = renderToStaticMarkup(
      <>
        <AgentMark agent="claude" working />
        <AgentMark agent="claude" working />
      </>,
    );
    const ids = [...two.matchAll(/<clipPath id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(2);
  });
  it("adds nothing for a Claude session that is not working, or for other vendors", () => {
    expect(renderToStaticMarkup(<AgentMark agent="claude" />)).not.toContain("agent-mark__core");
    expect(renderToStaticMarkup(<AgentMark agent="codex" working />)).not.toContain("agent-mark__core");
  });
});
