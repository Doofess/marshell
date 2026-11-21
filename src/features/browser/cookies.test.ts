import { describe, expect, it } from "vitest";
import { BROWSER_DEFAULTS } from "./browserSettings";
import { cookieChoice, cookieRows, cookiesSaid, needsReload, ruleToTell } from "./cookies";

describe("the cookie rule", () => {
  it("tells the saved rule, private pages when the settings can't be read, and nothing while they load", () => {
    expect(ruleToTell({ ...BROWSER_DEFAULTS, cookies: "until-quit" }, false)).toBe("until-quit");
    expect(ruleToTell(null, true)).toBe("never");
    expect(ruleToTell(null, false)).toBeNull();
  });
  it("loads pages again only when a page's privacy changes", () => {
    expect(needsReload(null, "never")).toBe(false);
    expect(needsReload("always", "until-quit")).toBe(false);
    expect(needsReload("always", "never")).toBe(true);
    expect(needsReload("never", "until-quit")).toBe(true);
    expect(needsReload("never", "never")).toBe(false);
  });
  it("words a change", () => {
    expect(cookiesSaid("until-quit", 0)).toBe("Cookies: Until quit.");
    expect(cookiesSaid("never", 1)).toBe("Cookies: Never. 1 tab was loaded again.");
    expect(cookiesSaid("never", 3)).toBe("Cookies: Never. 3 tabs were loaded again.");
    expect(cookiesSaid("always", 2)).toBe("Cookies: Always. 2 tabs were loaded again.");
  });
  it("lists three radio rows", () => {
    expect(cookieRows("never", true, false).map((r) => "label" in r && [r.id, r.label, r.checked, r.disabled])).toEqual([
      ["cookies:always", "Keep always", false, undefined],
      ["cookies:until-quit", "Keep until quit", false, undefined],
      ["cookies:never", "Never keep", true, undefined],
    ]);
    expect(cookieRows("always", false, false).every((r) => "label" in r && r.disabled === "Not available on this system yet")).toBe(true);
    expect(cookieRows(null, true, true).every((r) => "label" in r && r.checked === false && r.disabled === "Your settings couldn't be read")).toBe(true);
    expect(cookieRows(null, true, false).every((r) => "label" in r && r.checked === false && r.disabled === undefined)).toBe(true);
  });
  it("reads a row's id back", () => {
    expect(cookieChoice("cookies:until-quit")).toBe("until-quit");
    expect(cookieChoice("cookies:nope")).toBeUndefined();
    expect(cookieChoice("other")).toBeUndefined();
  });
});
