import { describe, expect, it } from "vitest";
import { focusOnChange, focusOnUnblock, terminalFocusProps } from "./terminalFocus";

describe("terminalFocusProps", () => {
  // A link's hold isn't an overlay: picking the terminal ends it, and the terminal takes the keyboard then (threat model, link hold).
  it("keeps the selected terminal from focusing while a link holds the keyboard, without blocking it", () => {
    expect(terminalFocusProps({ selected: true, linkHeld: true, overlayUp: false })).toEqual({ autoFocus: false, focusBlocked: false });
  });

  it("turns autoFocus on when a link's hold ends", () => {
    expect(terminalFocusProps({ selected: true, linkHeld: false, overlayUp: false }).autoFocus).toBe(true);
  });

  it("blocks the selected terminal while an overlay is up and leaves its autoFocus on", () => {
    expect(terminalFocusProps({ selected: true, linkHeld: false, overlayUp: true })).toEqual({ autoFocus: true, focusBlocked: true });
  });
});

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
