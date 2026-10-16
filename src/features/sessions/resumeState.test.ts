import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import { holdsResume, resumeDisabledReason } from "./resumeState";

const profile = (id: string) => ({ id }) as Profile;

describe("resumeDisabledReason", () => {
  it("names a profile that is missing from a loaded list", () => {
    expect(resumeDisabledReason("gone", [profile("pwsh")])).toBe("Its shell profile was removed.");
  });
  it("is silent for a profile that is in the list", () => {
    expect(resumeDisabledReason("pwsh", [profile("pwsh")])).toBeUndefined();
  });
  it("treats an empty list as unknown: a failed fetch is not a removed profile", () => {
    expect(resumeDisabledReason("pwsh", [])).toBeUndefined();
  });
});

describe("holdsResume", () => {
  const pressed = "Ended last time";
  it("holds while the request is out, whatever the live state says", () => {
    expect(holdsResume(pressed, pressed, "pending")).toBe(true);
    expect(holdsResume(pressed, "live", "pending")).toBe(true);
  });
  it("the ok response before the Running frame keeps holding, so a second press finds the strip", () => {
    expect(holdsResume(pressed, pressed, "ok")).toBe(true);
  });
  it("the ok response after the Running frame lets the live row decide", () => {
    expect(holdsResume(pressed, "live", "ok")).toBe(false);
  });
  it("a refusal drops the hold at once, and a rollback frame after it changes nothing", () => {
    expect(holdsResume(pressed, "live", "refused")).toBe(false);
    expect(holdsResume(pressed, pressed, "refused")).toBe(false);
  });
});
