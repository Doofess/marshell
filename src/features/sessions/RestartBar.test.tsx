import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RestartBar } from "./RestartBar";
import { closeBarKey } from "./CloseBar";

describe("RestartBar", () => {
  const h = renderToStaticMarkup(<RestartBar text="Restart 3 billing in Bypass?" onCancel={() => {}} onRestart={() => {}} />);
  it("is an alert that asks the question", () => {
    expect(h).toContain('role="alert"');
    expect(h).toContain("Restart 3 billing in Bypass?");
  });
  it("reuses the close bar's look", () => {
    expect(h).toContain('class="close-bar"');
  });
  it("offers Cancel first and Restart second, and only Cancel takes the focus", () => {
    expect(h.match(/<button/g)).toHaveLength(2);
    expect(h.match(/data-initial-focus/g)).toHaveLength(1);
    expect(h).toMatch(/data-initial-focus[^>]*>Cancel</);
    expect(h).toMatch(/>Restart</);
    expect(h.indexOf(">Cancel<")).toBeLessThan(h.indexOf(">Restart<"));
  });
  it("Escape cancels, by the same rule as the close bar", () => {
    expect(closeBarKey("Escape")).toBe("cancel");
  });
});
