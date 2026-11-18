import { describe, expect, it } from "vitest";
import { ADDRESS_BOX } from "../browser/address";
import { APP_KEYS_BOX, keepsOwnKeys } from "./reviewOpen";

const box = (tagName: string, ...classes: string[]) => ({ tagName, classList: { contains: (c: string) => classes.includes(c) } });

describe("keepsOwnKeys", () => {
  it("an_app_keys_box_gives_up_the_apps_shortcuts", () => {
    expect(APP_KEYS_BOX).toBe("app-keys");
    // The editor's content element is a div with role="textbox", so it is a box through its class.
    expect(keepsOwnKeys(box("DIV", APP_KEYS_BOX))).toBe(false);
    expect(keepsOwnKeys(box("TEXTAREA", APP_KEYS_BOX))).toBe(false);
    expect(keepsOwnKeys(box("INPUT", APP_KEYS_BOX))).toBe(false);
    // Unchanged: an ordinary box keeps its keys; the Address box gives them up.
    expect(keepsOwnKeys(box("INPUT"))).toBe(true);
    expect(keepsOwnKeys(box("INPUT", ADDRESS_BOX))).toBe(false);
  });
  it("an_app_keys_box_keeps_an_altgr_key", () => {
    expect(keepsOwnKeys(box("DIV", APP_KEYS_BOX), true)).toBe(true);
    expect(keepsOwnKeys(box("TEXTAREA", APP_KEYS_BOX), true)).toBe(true);
    expect(keepsOwnKeys(box("INPUT", APP_KEYS_BOX), true)).toBe(true);
    // The class on a button or a link does nothing.
    expect(keepsOwnKeys(box("BUTTON", APP_KEYS_BOX), true)).toBe(false);
  });
});
