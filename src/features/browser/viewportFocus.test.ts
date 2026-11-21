import { describe, expect, it } from "vitest";
import { viewportReturn } from "./viewportFocus";

type El = { name: string; isConnected: boolean };
const el = (name: string, isConnected = true): El => ({ name, isConnected });
const lookup = (chip: El | null, menu: El | null) => ({ chip: () => chip, menu: () => menu });

describe("viewportReturn", () => {
  const chip = el("front chip");
  const menu = el("Browser menu button");

  it("goes to the Browser menu button when Responsive was picked, since Responsive shows no chip", () => {
    expect(viewportReturn(null, chip, lookup(chip, menu))).toBe(menu);
  });
  it("goes back to the chip that opened the menu when a size was picked or nothing was", () => {
    expect(viewportReturn("phone", chip, lookup(null, menu))).toBe(chip);
    expect(viewportReturn(undefined, chip, lookup(null, menu))).toBe(chip);
  });
  it("looks for the front tab's own chip when the opener is gone, and then for the menu button", () => {
    const gone = el("old chip", false);
    expect(viewportReturn("tablet", gone, lookup(chip, menu))).toBe(chip);
    expect(viewportReturn("tablet", null, lookup(null, menu))).toBe(menu);
  });
  it("finds nothing when nothing is there", () => {
    expect(viewportReturn(null, null, lookup(null, null))).toBeNull();
  });
});
