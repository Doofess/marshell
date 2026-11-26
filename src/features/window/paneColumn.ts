import { createContext } from "react";
import { DRAWER, drawerPlacement, sidebarMode, sidebarWidth as widthFor, type SidebarMode, type SidebarPref } from "./layout";
import { PANE, paneGeometry } from "./paneLayout";

export type ColumnInputs = {
  pref: SidebarPref;
  sidebarWidth?: number;
  drawer: boolean;
  shown: boolean;
  saved: number | null;
  terminalMin: number;
  rtl: boolean;
  /** No session: an open pane fills the room (passed to paneGeometry). */
  alone?: boolean;
  /** The session's panel is expanded: the pane takes the terminal's place (passed to paneGeometry). */
  expanded?: boolean;
};

/** The main area in a window `width` wide: what MainWindow computes as mode, sideW, pushed and spaceW. */
export function mainRoom(
  width: number,
  i: Pick<ColumnInputs, "pref" | "sidebarWidth" | "drawer">,
): { mode: SidebarMode; side: number; pushed: number; room: number } {
  const mode = sidebarMode(width, i.pref);
  const side = widthFor(mode, i.sidebarWidth);
  const pushed = i.drawer && drawerPlacement(width, mode, DRAWER.default) === "push" ? DRAWER.default : 0;
  return { mode, side, pushed, room: width - side - pushed };
}

/**
 * The pane's column in window coordinates: the pane sits at the main area's end edge (its left in a right-to-left window).
 * Width 0 when hidden. The default width (half the room) is not rounded here, unlike the layout's: the page follows a straight
 * line through the window's sizes, and the layout's whole pixels sit within half a pixel of it. The app's own report corrects
 * the page once the window is still.
 */
export function paneColumn(width: number, i: ColumnInputs): { x: number; width: number } {
  const { side, pushed, room } = mainRoom(width, i);
  const g = paneGeometry({ room, shown: i.shown, saved: i.saved, terminalMin: i.terminalMin, alone: i.alone, expanded: i.expanded });
  let pane = g.pane;
  if (g.mode === "beside" && (i.saved === null || !Number.isFinite(i.saved))) {
    pane = Math.min(g.max, Math.max(PANE.min, (room - 1) / 2));
  }
  if (pane <= 0) return { x: i.rtl ? pushed : width - pushed, width: 0 };
  // The main area runs from `side` to `side + room` in a left-to-right window; the drawer is at the other end in a right-to-left one.
  return { x: i.rtl ? pushed : width - pushed - pane, width: pane };
}

/** What the window publishes for a page's slot: the inputs of the pane's column, and whether the layout is moving. Null outside the window. */
export type Layout = {
  inputs: ColumnInputs;
  moving: boolean;
  /** How many px the slot's distance from the window's foot grows per px of window height: the share of the stack below it. Absent: 0. */
  bottomPerPx?: number;
  /** Whether a pane that isn't expanded sits beside the terminal here: false under 720 px of room and with no session. Absent: true. */
  beside?: boolean;
};
export const LayoutContext = createContext<Layout | null>(null);
