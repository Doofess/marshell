import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PauseBar } from "./PauseBar";

describe("PauseBar", () => {
  const confirm = renderToStaticMarkup(<PauseBar text="Pause 3 billing?" action="Pause" onCancel={() => {}} onAction={() => {}} />);
  it("is an alert in the close bar's look", () => {
    expect(confirm).toContain('role="alert"');
    expect(confirm).toContain('class="close-bar"');
    expect(confirm).toContain("Pause 3 billing?");
  });
  it("offers Cancel first, then the action, and only Cancel takes the focus", () => {
    expect(confirm.match(/<button/g)).toHaveLength(2);
    expect(confirm.match(/data-initial-focus/g)).toHaveLength(1);
    expect(confirm.indexOf(">Cancel<")).toBeLessThan(confirm.indexOf(">Pause<"));
  });
  it("the typing bar's first button is Dismiss and its action is Continue", () => {
    const typing = renderToStaticMarkup(<PauseBar text="Paused. Continue to type." action="Continue" cancel="Dismiss" onCancel={() => {}} onAction={() => {}} />);
    expect(typing).toContain("Paused. Continue to type.");
    expect(typing.indexOf(">Dismiss<")).toBeLessThan(typing.indexOf(">Continue<"));
    expect(typing).not.toContain(">Cancel<");
  });
});
