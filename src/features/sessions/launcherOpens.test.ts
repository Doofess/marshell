import { describe, expect, it } from "vitest";
import { launcherOpens } from "./launcherOpens";

describe("launcherOpens", () => {
  it("opens for a core that has no sessions", () => {
    expect(launcherOpens(true, 0)).toBe(true);
  });

  it("stays closed until the core has sent its list", () => {
    expect(launcherOpens(false, 0)).toBe(false);
  });

  it("stays closed over restored tabs, such as after a core restart seeds them as Ended", () => {
    expect(launcherOpens(true, 3)).toBe(false);
  });
});
