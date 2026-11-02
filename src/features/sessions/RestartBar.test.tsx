import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { RestartBar, restartAllowed } from "./RestartBar";
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
  it("can confirm a deferred administrator Reopen with the same checkbox", () => {
    const h = renderToStaticMarkup(<RestartBar text="Reopen the session?" actionLabel="Reopen as administrator" needsAck onCancel={() => {}} onRestart={() => {}} />);
    expect(h).toContain('type="checkbox"');
    expect(h).toMatch(/aria-disabled="true"[^>]*>Reopen as administrator</);
  });
  it("Escape cancels, by the same rule as the close bar", () => {
    expect(closeBarKey("Escape")).toBe("cancel");
  });
});

describe("RestartBar: the administrator confirmation", () => {
  const withAck = (p: Partial<Parameters<typeof RestartBar>[0]> = {}) => renderToStaticMarkup(<RestartBar text="Restart 3 infra in Bypass?" needsAck onCancel={() => {}} onRestart={() => {}} {...p} />);
  it("bypass_on_an_elevated_row_needs_the_checkbox: a labelled checkbox, and Restart disabled until it is ticked", () => {
    const h = withAck();
    expect(h).toContain("I understand this agent runs as administrator and Bypass lets it run admin commands without asking.");
    expect(h).toMatch(/<input[^>]*type="checkbox"/);
    expect(h).toMatch(/aria-disabled="true"[^>]*>Restart</);
    expect(h).not.toMatch(/type="checkbox"[^>]*tabindex="-1"/);
  });
  it("shows no checkbox for an ordinary restart, and Restart is enabled", () => {
    const h = renderToStaticMarkup(<RestartBar text="Restart?" onCancel={() => {}} onRestart={() => {}} />);
    expect(h).not.toContain("checkbox");
    expect(h).not.toContain("aria-disabled");
  });
  it("Cancel still takes the focus first", () => {
    expect(withAck()).toMatch(/data-initial-focus[^>]*>Cancel</);
  });
  it("restart is allowed only when no ack is needed or it is ticked", () => {
    expect(restartAllowed(false, false)).toBe(true);
    expect(restartAllowed(true, false)).toBe(false);
    expect(restartAllowed(true, true)).toBe(true);
  });
});
