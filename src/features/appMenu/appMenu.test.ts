import { describe, expect, it } from "vitest";
import { appMenuItems, doctorItemLabel, leavesAppMenu, menuKeyCloses, menuPlace, SETTINGS_ITEMS, submenuPlace, toggleMenu, type MenuState } from "./appMenu";

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
  it("a_press_on_the_menu_button_leaves_the_closing_to_its_click", () => {
    const inside = { name: "item" };
    const button = { name: "menu button" };
    const elsewhere = { name: "terminal" };
    const menu = { contains: (n: never) => n === (inside as never) };
    const isTrigger = (n: object) => n === button;
    expect(leavesAppMenu(elsewhere, menu, isTrigger)).toBe(true);
    expect(leavesAppMenu(button, menu, isTrigger)).toBe(false);
    expect(leavesAppMenu(inside, menu, isTrigger)).toBe(false);
    expect(leavesAppMenu(null, menu, isTrigger)).toBe(false);
  });
  it("the_menu_key_pressed_in_the_menu_closes_it", () => {
    const key = (code: string, mods: { ctrlKey?: boolean; shiftKey?: boolean; metaKey?: boolean } = {}) => ({ code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods });
    expect(menuKeyCloses(key("KeyM", { ctrlKey: true, shiftKey: true }), "windows")).toBe(true);
    expect(menuKeyCloses(key("KeyE", { metaKey: true }), "mac")).toBe(true);
    expect(menuKeyCloses(key("KeyM"), "windows")).toBe(false);
    expect(menuKeyCloses(key("KeyY", { ctrlKey: true, shiftKey: true }), "windows")).toBe(false);
  });
});
