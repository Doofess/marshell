import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("says what is empty, one way forward, and has no live region", () => {
    const h = renderToStaticMarkup(<EmptyState title="No coding agents found" body="Install Claude Code, Codex or Antigravity, or continue with shells and tools." action={{ label: "Continue with shells and tools" }} />);
    expect(h).toContain("No coding agents found");
    expect(h).toContain("Continue with shells and tools");
    expect(h).not.toMatch(/role="status"|aria-live|role="alert"/);
  });
  it("renders without an action", () => {
    expect(renderToStaticMarkup(<EmptyState title="All clear" body="Nothing needs fixing." />)).not.toContain("<button");
  });
});
