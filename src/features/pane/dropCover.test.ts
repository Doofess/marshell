import { describe, expect, it } from "vitest";
import { dropCover } from "./dropCover";

const none = { launcherOpen: false, overlay: null, review: false, closeAsk: false };

describe("dropCover", () => {
  it("names the launcher", () => {
    expect(dropCover({ ...none, launcherOpen: true })).toBe("the launcher");
  });
  it("names the command palette and Settings", () => {
    expect(dropCover({ ...none, overlay: "palette" })).toBe("the command palette");
    expect(dropCover({ ...none, overlay: "settings" })).toBe("Settings");
  });
  it("calls any other overlay, the review sheet and the close bar the dialog", () => {
    expect(dropCover({ ...none, overlay: "doctor" })).toBe("the dialog");
    expect(dropCover({ ...none, review: true })).toBe("the dialog");
    expect(dropCover({ ...none, closeAsk: true })).toBe("the dialog");
  });
  it("is null with nothing over the window", () => {
    expect(dropCover(none)).toBeNull();
  });
  it("lets the launcher win over an overlay", () => {
    expect(dropCover({ ...none, launcherOpen: true, overlay: "settings" })).toBe("the launcher");
  });
});
