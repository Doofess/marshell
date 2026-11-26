import { describe, expect, it } from "vitest";
import { expandButton } from "./expandButton";

const b = (on: boolean, others: boolean, beside: boolean) => expandButton("files", { on, others, beside });

describe("expandButton", () => {
  it("is named Expand and the panel, whatever its state", () => {
    for (const on of [false, true]) {
      for (const others of [false, true]) {
        for (const beside of [false, true]) expect(b(on, others, beside).label).toBe("Expand Files");
      }
    }
    expect(expandButton("changes", { on: false, others: true, beside: true }).label).toBe("Expand Changes");
    expect(expandButton("browser", { on: false, others: true, beside: true }).label).toBe("Expand Browser");
  });
  it("with no other panel and no terminal beside there is nothing to change: disabled, and it says why", () => {
    for (const on of [false, true]) {
      expect(b(on, false, false)).toEqual({ label: "Expand Files", pressed: false, disabled: true, title: "No other panel is shown" });
    }
  });
  it("off, it expands (a lone panel beside the terminal can still take the terminal's place)", () => {
    expect(b(false, true, true)).toEqual({ label: "Expand Files", pressed: false, disabled: false, title: "Expand Files" });
    expect(b(false, true, false)).toEqual({ label: "Expand Files", pressed: false, disabled: false, title: "Expand Files" });
    expect(b(false, false, true)).toEqual({ label: "Expand Files", pressed: false, disabled: false, title: "Expand Files" });
    expect(expandButton("changes", { on: false, others: true, beside: true }).title).toBe("Expand Changes");
  });
  it("on, the tooltip says what pressing it brings back", () => {
    expect(b(true, true, true)).toEqual({ label: "Expand Files", pressed: true, disabled: false, title: "Show the terminal and the other panels" });
    expect(b(true, false, true)).toEqual({ label: "Expand Files", pressed: true, disabled: false, title: "Show the terminal" });
    expect(b(true, true, false)).toEqual({ label: "Expand Files", pressed: true, disabled: false, title: "Show the other panels" });
  });
});
