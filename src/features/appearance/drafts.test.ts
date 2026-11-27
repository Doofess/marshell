import { describe, expect, it } from "vitest";
import { draftsOf, refill } from "./drafts";
import { DEFAULT_APPEARANCE } from "./rootState";

const loaded = { ...DEFAULT_APPEARANCE, terminal_font: { family: "Cascadia Mono", size: 16 } };

describe("refill", () => {
  it("fields opened before the settings loaded pick up the loaded values", () => {
    const before = draftsOf(DEFAULT_APPEARANCE);
    expect(before.font).toBe("");
    expect(before.size.text).toBe("13");
    const after = refill(before, loaded, null);
    expect(after.font).toBe("Cascadia Mono");
    expect(after.size.text).toBe("16");
  });

  it("leaves the field that has focus alone", () => {
    const typing = { ...draftsOf(DEFAULT_APPEARANCE), font: "Fira" };
    const after = refill(typing, loaded, "font");
    expect(after.font).toBe("Fira");
    expect(after.size.text).toBe("16");
  });

  it("holds only the font and the size", () => expect(Object.keys(draftsOf(DEFAULT_APPEARANCE)).sort()).toEqual(["font", "size"]));
});
