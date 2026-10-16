import { describe, expect, it } from "vitest";
import { blurAccent, commitAccent, draftsOf, pickPreset, refill } from "./drafts";
import { DEFAULT_APPEARANCE } from "./rootState";

const loaded = { ...DEFAULT_APPEARANCE, accent: "#3a7bd5", terminal_font: { family: "Cascadia Mono", size: 16 } };

describe("refill", () => {
  it("fields opened before the settings loaded pick up the loaded values", () => {
    const before = draftsOf(DEFAULT_APPEARANCE);
    expect(before.font).toBe("");
    expect(before.size.text).toBe("13");
    const after = refill(before, loaded, null);
    expect(after.accent.text).toBe("#3a7bd5");
    expect(after.font).toBe("Cascadia Mono");
    expect(after.size.text).toBe("16");
  });

  it("leaves the field that has focus alone", () => {
    const typing = { ...draftsOf(DEFAULT_APPEARANCE), font: "Fira" };
    const after = refill(typing, loaded, "font");
    expect(after.font).toBe("Fira");
    expect(after.size.text).toBe("16");
  });
});

const typed = (text: string) => ({ ...draftsOf(DEFAULT_APPEARANCE), accent: { text, reason: null } });

describe("custom accent on blur", () => {
  it("unchanged text commits nothing", () => {
    const filled = draftsOf({ ...DEFAULT_APPEARANCE, accent: "#3a7bd5" });
    expect(blurAccent(filled).hex).toBeNull();
    // Nor does text that was committed already.
    const first = commitAccent(typed("3A7BD5"));
    expect(first.hex).toBe("#3a7bd5");
    expect(blurAccent(first.drafts).hex).toBeNull();
  });

  it("text filled in by a load commits nothing", () => {
    const after = refill(draftsOf(DEFAULT_APPEARANCE), loaded, null);
    expect(blurAccent(after).hex).toBeNull();
  });

  it("empty text does nothing and shows no error", () => {
    const out = blurAccent(typed(""));
    expect(out.hex).toBeNull();
    expect(out.drafts.accent.reason).toBeNull();
  });

  it("changed text is committed, and a bad value says why", () => {
    expect(blurAccent(typed("#3a7bd5")).hex).toBe("#3a7bd5");
    const bad = blurAccent(typed("zzz"));
    expect(bad.hex).toBeNull();
    expect(bad.drafts.accent.reason).toMatch(/#rrggbb/);
  });

  it("Enter or Custom with empty text still says what to enter", () => {
    expect(commitAccent(typed("")).drafts.accent.reason).toMatch(/#rrggbb/);
  });

  it("picking a preset clears the custom text", () => {
    const out = pickPreset(draftsOf({ ...DEFAULT_APPEARANCE, accent: "#3a7bd5" }));
    expect(out.accent).toEqual({ text: "", reason: null });
    expect(blurAccent(out).hex).toBeNull();
  });
});
