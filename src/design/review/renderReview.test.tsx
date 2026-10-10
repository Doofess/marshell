import { describe, expect, it, vi } from "vitest";
import { renderReview } from "./renderReview";

// The Colours story parses the CSS it imports with ?raw, which Vitest hands over as empty strings, so it is stubbed here;
// the real page is built and checked by `pnpm review:build`.
vi.mock("../Colours.stories", () => ({ default: { title: "Design / Colours" }, Palette: { render: () => null } }));

describe("renderReview", () => {
  it("is cumulative: 2a contains batch 1 and the approve card", () => {
    const html = renderReview("2a");
    expect(html).toContain('id="sidebar-session-rows"');
    expect(html).toContain('id="approve-card"');
  });
  it("batch 1 alone has no approve card", () => {
    expect(renderReview("1")).not.toContain('id="approve-card"');
  });
  it("renders every approve card story", () => {
    const html = renderReview("2a");
    for (const name of ["Safe bash", "Risky bash", "Edit with diff", "Write new file", "Mcp tool", "Long command", "Question card", "After decision", "Answered in terminal", "Released on timeout"])
      expect(html, name).toContain(name);
  });
});
