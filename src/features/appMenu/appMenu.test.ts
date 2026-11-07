import { describe, expect, it } from "vitest";
import { appMenuItems, doctorItemLabel, menuPlace, SETTINGS_ITEMS, submenuPlace, toggleMenu, type MenuState } from "./appMenu";

describe("appMenu", () => {
  it("the_menu_lists_settings_history_doctor_departures", () => {
    const items = appMenuItems("windows");
    expect(items.map((i) => ("label" in i ? i.label : "-"))).toEqual(["Settings", "History", "Doctor", "Departures"]);
    expect(items[0]).toMatchObject({ submenu: true });
    expect(items[1]).toMatchObject({ shortcut: "Ctrl+Shift+Y" });
  });
  it("settings_lists_its_six_sections", () => {
    expect(SETTINGS_ITEMS.map((s) => s.label)).toEqual(["Appearance", "Agents", "Shell profiles", "Notifications", "History settings", "Diagnostics"]);
  });
  it("doctor_reads_its_count_and_no_number_before_a_check", () => {
    expect(doctorItemLabel(undefined)).toBe("Doctor");
    expect(doctorItemLabel(0)).toBe("Doctor, no issues");
    expect(doctorItemLabel(1)).toBe("Doctor, 1 issue");
    expect(doctorItemLabel(3)).toBe("Doctor, 3 issues");
  });
  it("a_menu_with_no_anchor_opens_at_the_bottom_left", () => {
    expect(menuPlace(null, { w: 200, h: 160 }, { w: 1200, h: 800 })).toEqual({ x: 8, y: 632 });
  });
  it("a_menu_opens_above_its_button", () => {
    expect(menuPlace({ left: 14, top: 760 }, { w: 200, h: 160 }, { w: 1200, h: 800 })).toEqual({ x: 14, y: 596 });
  });
  it("the_submenu_opens_beside_the_menu_or_on_the_other_side", () => {
    const vp = { w: 1200, h: 800 };
    const size = { w: 180, h: 200 };
    expect(submenuPlace({ left: 14, right: 214 }, 600, size, vp)).toEqual({ x: 216, y: 592 });
    expect(submenuPlace({ left: 1000, right: 1190 }, 100, size, vp)).toEqual({ x: 818, y: 100 });
  });
});

describe("toggleMenu", () => {
  it("the_menu_key_toggles_and_remembers_where_focus_was", () => {
    const el = { focus() {}, isConnected: true };
    const open = toggleMenu(null, { anchor: null, back: el });
    expect(open).toEqual({ anchor: null, back: el });
    expect(toggleMenu(open as MenuState, { anchor: null, back: el })).toBeNull();
  });
});
