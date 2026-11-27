import { describe, expect, it } from "vitest";
import { menuKey } from "../../components/ContextMenu/menuKeys";
import { appMenuItems, appMenuKey, doctorItemLabel, isMenuTrigger, leavesAppMenu, menuAnchor, menuKeyCloses, menuPlace, pressMenuButton, SETTINGS_ITEMS, toggleMenu, type MenuState } from "./appMenu";

describe("appMenu", () => {
  it("the_menu_lists_settings_history_doctor_instructions_departures", () => {
    const items = appMenuItems("windows");
    expect(items.map((i) => ("label" in i ? i.label : "-"))).toEqual(["Settings", "History", "Doctor", "Instructions", "Departures"]);
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
  it("a_menu_opened_from_the_title_bar_opens_under_its_button", () => {
    expect(menuPlace({ left: 12, top: 8, bottom: 36 }, { w: 200, h: 160 }, { w: 1200, h: 800 })).toEqual({ x: 12, y: 40 });
  });
  it("a_menu_under_a_button_still_stays_inside_the_window", () => {
    expect(menuPlace({ left: 12, top: 760, bottom: 790 }, { w: 200, h: 160 }, { w: 1200, h: 800 })).toEqual({ x: 12, y: 632 });
  });
});

describe("menuAnchor", () => {
  const rect = { left: 12, top: 8, bottom: 36 };
  it("the_sidebar_and_rail_buttons_anchor_by_their_top_so_the_menu_opens_above", () => {
    expect(menuAnchor(rect, false)).toEqual({ left: 12, top: 8 });
  });
  it("the_title_bar_brand_anchors_by_its_bottom_so_the_menu_opens_under_it", () => {
    expect(menuAnchor(rect, true)).toEqual({ left: 12, top: 8, bottom: 36 });
  });
});

describe("isMenuTrigger", () => {
  const el = (classes: string[]) => ({ closest: (sel: string) => (classes.some((c) => sel.split(",").map((s) => s.trim()).includes(`.${c}`)) ? {} : null) });
  it("the_sidebar_rail_and_brand_buttons_toggle_the_menu", () => {
    for (const c of ["sidebar__menu", "rail__menu", "window__brand"]) expect(isMenuTrigger(el([c])), c).toBe(true);
  });
  it("nothing_else_does", () => {
    expect(isMenuTrigger(el(["window__palette"]))).toBe(false);
    expect(isMenuTrigger({})).toBe(false);
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
  it("holding_the_menu_key_does_not_close_the_menu_it_just_opened", () => {
    const held = { code: "KeyM", ctrlKey: true, shiftKey: true, altKey: false, metaKey: false, repeat: true };
    expect(menuKeyCloses(held, "windows")).toBe(false);
  });
  it("a_click_on_the_button_of_an_open_menu_closes_it_and_leaves_the_focus_on_the_button", () => {
    // While the button is held, the menu's focus guard pulls the focus back into the menu; the click must hand it to the button.
    const button = { focus() {}, isConnected: true };
    const request = { anchor: { left: 14, top: 747 }, back: button };
    expect(pressMenuButton(null, request)).toEqual({ menu: request, focus: null });
    expect(pressMenuButton(request, request)).toEqual({ menu: null, focus: button });
  });
});

describe("appMenuKey", () => {
  const top = appMenuItems("windows");
  const sub = SETTINGS_ITEMS.map((s) => ({ id: s.section, label: s.label }));
  it("tab_closes_the_menu_and_puts_the_focus_back_where_it_was_opened", () => {
    // Moving on from there would leave the page when the menu came from the terminal, the last stop in the Tab order.
    expect(appMenuKey(menuKey("Tab", top, 0), "top")).toEqual({ do: "close" });
    expect(appMenuKey(menuKey("Tab", sub, 2), "sub")).toEqual({ do: "close" });
    expect(appMenuKey(menuKey("Escape", top, 0), "top")).toEqual({ do: "close" });
  });
  it("left_does_nothing_at_the_top_and_left_or_esc_go_up_from_the_submenu", () => {
    expect(appMenuKey(menuKey("ArrowLeft", top, 1), "top")).toBeNull();
    expect(appMenuKey(menuKey("ArrowLeft", sub, 0), "sub")).toEqual({ do: "up" });
    expect(appMenuKey(menuKey("Escape", sub, 0), "sub")).toEqual({ do: "up" });
  });
  it("moves_opens_and_runs_as_the_menu_keys_say", () => {
    expect(appMenuKey(menuKey("ArrowDown", top, 0), "top")).toEqual({ do: "move", index: 1 });
    expect(appMenuKey(menuKey("Enter", top, 0), "top")).toEqual({ do: "open" });
    expect(appMenuKey(menuKey("Enter", top, 1), "top")).toEqual({ do: "run", id: "history" });
    expect(appMenuKey(menuKey("Enter", sub, 2), "sub")).toEqual({ do: "run", id: "profiles" });
    expect(appMenuKey(menuKey("x", top, 0), "top")).toBeNull();
  });
});
