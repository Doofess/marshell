import { placeMenu, type MenuItem } from "../../components/ContextMenu/menuKeys";
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
