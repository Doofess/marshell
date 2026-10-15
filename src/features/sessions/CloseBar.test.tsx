import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CloseBar, closeBarKey } from "./CloseBar";

describe("CloseBar", () => {
  const h = renderToStaticMarkup(<CloseBar text="Codex may be mid-task in api." onCancel={() => {}} onCloseNow={() => {}} />);
  it("is an alert that says who may be mid-task and where", () => {
    expect(h).toContain('role="alert"');
    expect(h).toContain("Codex may be mid-task in api.");
  });
  it("offers Cancel first and Close now second, and only Cancel takes the focus", () => {
    expect(h.match(/<button/g)).toHaveLength(2);
    expect(h.match(/data-initial-focus/g)).toHaveLength(1);
    expect(h).toMatch(/data-initial-focus[^>]*>Cancel</);
    expect(h).toMatch(/>Close now</);
  });
  it("Escape cancels; other keys do nothing", () => {
    expect(closeBarKey("Escape")).toBe("cancel");
    expect(closeBarKey("Enter")).toBeNull();
    expect(closeBarKey("Tab")).toBeNull();
  });
});
