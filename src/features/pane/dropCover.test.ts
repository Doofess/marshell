import { describe, expect, it } from "vitest";
import { dropCover, modalOnScreen } from "./dropCover";

const none = { launcherOpen: false, overlay: null, review: false, closeAsk: false, modal: false };

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
  it("calls any other modal layer the dialog: the scripts question, the first-run setup", () => {
    expect(dropCover({ ...none, modal: true })).toBe("the dialog");
  });
  it("finds a modal layer by its flag, and the first-run setup by its layer", () => {
    const seen: string[] = [];
    const doc = (found: boolean) => ({ querySelector: (q: string) => (seen.push(q), found ? ({} as Element) : null) });
    expect(modalOnScreen(doc(true))).toBe(true);
    expect(modalOnScreen(doc(false))).toBe(false);
    expect(seen[0]).toContain('[aria-modal="true"]');
    expect(seen[0]).toContain(".live-app__onboarding");
  });
  it("is null with nothing over the window", () => {
    expect(dropCover(none)).toBeNull();
  });
  it("lets the launcher win over an overlay", () => {
    expect(dropCover({ ...none, launcherOpen: true, overlay: "settings" })).toBe("the launcher");
  });
});
