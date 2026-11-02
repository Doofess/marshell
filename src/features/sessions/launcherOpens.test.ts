import { describe, expect, it } from "vitest";
import { adminCommandHidden, launcherFromPalette, launcherOpens } from "./launcherOpens";
import { COMMANDS } from "../../lib/keymap";
import { LIVE_COMMANDS } from "./LivePalette";

describe("launcherOpens", () => {
  it("opens for a core that has no sessions", () => {
    expect(launcherOpens(true, 0, "done")).toBe(true);
  });

  it("stays closed for a Reopen confirmation and opens again after it ends", () => {
    expect(launcherOpens(true, 0, "done", true)).toBe(false);
    expect(launcherOpens(true, 0, "done", false)).toBe(true);
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

describe("launcherFromPalette", () => {
  it("the_palette_command_opens_the_launcher_with_admin_on", () => {
    expect(launcherFromPalette("new-session-admin")).toEqual({ elevated: true });
    expect(launcherFromPalette("launcher")).toEqual({ elevated: false });
    expect(launcherFromPalette("close")).toBeNull();
  });
  it("is hidden off Windows and listed on it, labelled as the copy says", () => {
    expect(adminCommandHidden("mac")).toBe(true);
    expect(adminCommandHidden("linux")).toBe(true);
    expect(adminCommandHidden("windows")).toBe(false);
    expect(adminCommandHidden(undefined)).toBe(true);
    expect(COMMANDS.find((c) => c.id === "new-session-admin")?.label).toBe("New session as administrator");
    expect(LIVE_COMMANDS.map((c) => c.id)).toContain("new-session-admin");
  });
});
