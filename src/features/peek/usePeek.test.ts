import { describe, expect, it } from "vitest";
import { clampTop, hoverDelay, peekName } from "./usePeek";

describe("usePeek helpers", () => {
  it("opens on hover after 350 ms", () => {
    expect(hoverDelay).toBe(350);
  });
  it("names the dialog by session number and name", () => {
    expect(peekName({ number: 6, name: "api-server" })).toBe("Peek: Session 6, api-server");
  });
  it("keeps the popover inside the window", () => {
    expect(clampTop(100, 800, 300)).toBe(100);
    expect(clampTop(700, 800, 300)).toBe(492);
    expect(clampTop(10, 200, 300)).toBe(8);
  });
});
