import { isValidElement, type ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { Announcer } from "./Announcer";

describe("Announcer", () => {
  it("replaces its text node for each message, so a repeat of the same words is read again", () => {
    // A screen reader reads a live region when its content changes; the same string re-rendered changes nothing.
    const first = Announcer({ message: { text: "Selected session 4, api", priority: "polite", seq: 3 } });
    const again = Announcer({ message: { text: "Selected session 4, api", priority: "polite", seq: 4 } });
    const inner = (el: ReactElement) => (el.props as { children: unknown }).children as ReactElement;
    expect(isValidElement(inner(first))).toBe(true);
    expect(inner(first).key).not.toBe(inner(again).key);
  });
  it("renders nothing inside until something is said", () => {
    const empty = Announcer({ message: { text: "", priority: "polite", seq: 0 } });
    expect((empty.props as { children: unknown }).children).toBeFalsy();
  });
});
