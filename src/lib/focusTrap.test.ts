import { describe, expect, it } from "vitest";
import { trapKey } from "./focusTrap";

describe("trapKey: a dialog keeps Tab inside it and closes on Escape", () => {
  it("Tab moves to the next control and wraps from the last to the first", () => {
    expect(trapKey({ key: "Tab" }, 3, 0)).toEqual({ kind: "focus", index: 1 });
    expect(trapKey({ key: "Tab" }, 3, 2)).toEqual({ kind: "focus", index: 0 });
  });
  it("Shift+Tab moves back and wraps from the first to the last", () => {
    expect(trapKey({ key: "Tab", shiftKey: true }, 3, 1)).toEqual({ kind: "focus", index: 0 });
    expect(trapKey({ key: "Tab", shiftKey: true }, 3, 0)).toEqual({ kind: "focus", index: 2 });
  });
  it("from the title (focus is on none of the controls) Tab goes to the first control and Shift+Tab to the last", () => {
    expect(trapKey({ key: "Tab" }, 3, -1)).toEqual({ kind: "focus", index: 0 });
    expect(trapKey({ key: "Tab", shiftKey: true }, 3, -1)).toEqual({ kind: "focus", index: 2 });
  });
  it("with no control inside, Tab stays where it is", () => {
    expect(trapKey({ key: "Tab" }, 0, -1)).toEqual({ kind: "stay" });
  });
  it("Escape closes, and any other key is left alone", () => {
    expect(trapKey({ key: "Escape" }, 3, 1)).toEqual({ kind: "close" });
    expect(trapKey({ key: "Enter" }, 3, 1)).toBeNull();
    expect(trapKey({ key: "a" }, 3, 1)).toBeNull();
  });
});
