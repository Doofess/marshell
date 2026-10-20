import { describe, expect, it } from "vitest";
import { launcherOpens } from "./launcherOpens";

describe("launcherOpens", () => {
  it("opens for a core that has no sessions", () => {
    expect(launcherOpens(true, 0, "done")).toBe(true);
  });

  it("stays closed until the core has sent its list", () => {
    expect(launcherOpens(false, 0, "done")).toBe(false);
  });

  it("stays closed over restored tabs, such as after a core restart seeds them as Ended", () => {
    expect(launcherOpens(true, 3, "done")).toBe(false);
  });

  // Issue #37: both opened on a first run, the launcher took the focus, and the Enter meant for onboarding started Claude.
  it("stays closed while the onboarding is shown, and until the window knows whether it will be", () => {
    expect(launcherOpens(true, 0, "showing")).toBe(false);
    expect(launcherOpens(true, 0, "unknown")).toBe(false);
  });
});
