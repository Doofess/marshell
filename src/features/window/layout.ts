/** Sizes and rules from docs/PLAN.md "Layout". */
export const SIDEBAR = { default: 288, min: 200, max: 400, rail: 52 } as const;
export const AUTO_COLLAPSE_BELOW = 960;
export const MIN_WINDOW = { width: 720, height: 480 } as const;
export const DRAWER = { default: 360, min: 300, max: 560 } as const;

export type SidebarPref = "expanded" | "rail" | "focus";
export type SidebarMode = "expanded" | "rail" | "focus";

/** Below 960 px the expanded sidebar collapses to the rail by itself; the user's rail and focus choices always stand. */
export function sidebarMode(windowWidth: number, pref: SidebarPref): SidebarMode {
  if (pref !== "expanded") return pref;
  return windowWidth < AUTO_COLLAPSE_BELOW ? "rail" : "expanded";
}

export function sidebarWidth(mode: SidebarMode, requested: number = SIDEBAR.default): number {
  if (mode === "focus") return 0;
  if (mode === "rail") return SIDEBAR.rail;
  const r = Number.isFinite(requested) ? requested : SIDEBAR.default;
  return Math.min(SIDEBAR.max, Math.max(SIDEBAR.min, r));
}

/** The drawer pushes the terminal, and overlays it instead when the terminal would drop below 720 px. */
export function drawerPlacement(windowWidth: number, mode: SidebarMode, drawerWidth: number = DRAWER.default): "push" | "overlay" {
  return windowWidth - sidebarWidth(mode) - drawerWidth >= MIN_WINDOW.width ? "push" : "overlay";
}
