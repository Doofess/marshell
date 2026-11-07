import { leavesMenu, placeMenu, type MenuItem, type MenuKeyResult } from "../../components/ContextMenu/menuKeys";
import { sessionShortcut } from "../../lib/appShortcuts";
import type { KeyEventLike } from "../../lib/bindings";
import { commandById, shortcutLabel, type Os } from "../../lib/keymap";
import type { SettingsSection } from "../settings/settingsSections";

export type AppMenuRun = "history" | "doctor" | "departures" | `settings:${SettingsSection}`;

export const SETTINGS_ITEMS: ReadonlyArray<{ section: SettingsSection; label: string }> = [
  { section: "appearance", label: "Appearance" },
  { section: "agents", label: "Agents" },
  { section: "profiles", label: "Shell profiles" },
  { section: "notifications", label: "Notifications" },
  { section: "history", label: "History settings" },
  { section: "advanced", label: "Diagnostics" },
];

/** The menu's top level. History shows its shortcut for the platform. */
export function appMenuItems(os: Os): MenuItem[] {
  const binding = commandById("history").binding;
  return [
    { id: "settings", label: "Settings", submenu: true },
    { id: "history", label: "History", shortcut: binding ? shortcutLabel(binding, os) : undefined },
    { id: "doctor", label: "Doctor" },
    { id: "departures", label: "Departures" },
  ];
}

/** What a screen reader hears for Doctor. An unknown count says no number. */
export function doctorItemLabel(issues: number | undefined): string {
  if (issues === undefined) return "Doctor";
  if (issues === 0) return "Doctor, no issues";
  return `Doctor, ${issues} ${issues === 1 ? "issue" : "issues"}`;
}

type Size = { w: number; h: number };

/** Above its button, or at the window's bottom left when it has no button to sit on. */
export function menuPlace(anchor: { left: number; top: number } | null, size: Size, viewport: Size): { x: number; y: number } {
  return anchor
    ? placeMenu({ x: anchor.left, y: anchor.top - size.h - 4 }, size, viewport)
    : placeMenu({ x: 8, y: viewport.h - size.h - 8 }, size, viewport);
}

/** Beside the menu on its right, or on its left when that would leave the window; level with the item, kept on screen. */
export function submenuPlace(menu: { left: number; right: number }, itemTop: number, size: Size, viewport: Size): { x: number; y: number } {
  const right = menu.right + 2;
  const x = right + size.w + 8 <= viewport.w ? right : menu.left - size.w - 2;
  return placeMenu({ x, y: itemTop }, size, viewport);
}

/** The open menu: where it sits (null when no button is on screen) and what had the focus before it opened. */
export type MenuState = { anchor: { left: number; top: number } | null; back: { focus(): void; isConnected: boolean } | null } | null;

/** The menu key: closes an open menu, else opens with `request`. */
export function toggleMenu(open: MenuState, request: NonNullable<MenuState>): MenuState {
  return open ? null : request;
}

/** A press or a focus move to `next` leaves the menu, unless it lands on the button that toggles it: that button's own click closes the menu. */
export function leavesAppMenu(next: object | null, menu: { contains: (n: never) => boolean }, isTrigger: (n: object) => boolean): boolean {
  return leavesMenu(next, menu) && !isTrigger(next!);
}

/** The menu key (Ctrl+Shift+M, ⌘E, or what it was rebound to) pressed inside the menu closes it, as it opened it. Its repeats don't: the window ignores them, so a held key leaves the menu open. */
export function menuKeyCloses(e: KeyEventLike & { repeat?: boolean }, os: Os): boolean {
  return !e.repeat && sessionShortcut(e, os)?.kind === "menu";
}

/** A click on the Menu button opens the menu from it, or closes the open menu and gives the button the focus: the menu's focus guard took it back into the menu while the button was held. */
export function pressMenuButton(open: MenuState, request: NonNullable<MenuState>): { menu: MenuState; focus: NonNullable<MenuState>["back"] } {
  return open ? { menu: null, focus: request.back } : { menu: request, focus: null };
}

export type AppMenuKey = { do: "move"; index: number } | { do: "open" } | { do: "run"; id: string } | { do: "up" } | { do: "close"; keepDefault: boolean } | null;

/**
 * What a key does in the menu (`top`) or its Settings submenu (`sub`). Left and Esc in the submenu go up to the menu; Left in
 * the menu does nothing, as in a native menu. Esc closes the menu and puts the focus back where it was. Tab and Shift+Tab do
 * the same and keep their default, so the focus moves on from there and never falls to the page.
 */
export function appMenuKey(r: MenuKeyResult, level: "top" | "sub"): AppMenuKey {
  if (!r) return null;
  switch (r.kind) {
    case "move":
      return { do: "move", index: r.index };
    case "open":
      return level === "top" ? { do: "open" } : null;
    case "run":
      return { do: "run", id: r.id };
    case "back":
      return level === "sub" ? { do: "up" } : null;
    case "close":
      return level === "sub" && r.refocus ? { do: "up" } : { do: "close", keepDefault: !r.refocus };
  }
}
