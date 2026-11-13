import { describe, expect, it } from "vitest";
import { focusOnChange, focusOnUnblock } from "./terminalFocus";

describe("focusOnChange", () => {
  it("focuses the selected session's terminal when nothing blocks it", () => {
    expect(focusOnChange({ selected: true, blocked: false })).toBe(true);
  });

  it("leaves the keyboard alone while an overlay blocks", () => {
    expect(focusOnChange({ selected: true, blocked: true })).toBe(false);
  });

  it("leaves a terminal that is not selected alone", () => {
    expect(focusOnChange({ selected: false, blocked: false })).toBe(false);
  });
});

describe("focusOnUnblock", () => {
  it("leaves the keyboard to the terminal when the overlay restored nothing", () => {
    expect(focusOnUnblock({ selected: true, onNothing: true })).toBe(true);
  });

  it("keeps the focus an overlay restored", () => {
    expect(focusOnUnblock({ selected: true, onNothing: false })).toBe(false);
  });

  it("does nothing for a terminal that is not selected", () => {
    expect(focusOnUnblock({ selected: false, onNothing: true })).toBe(false);
  });
});
