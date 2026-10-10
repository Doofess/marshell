import type { NavContext, NavLayout } from "./types";

/**
 * A window as the navigation layer sees it: a top bar (brand, palette, layout button), a sidebar with five sessions
 * (numbered 1, 2, 3, 5, 6: session 4 was closed), and four panes in a 2 by 2 grid. Used by the tests and the stories.
 */
export const GRID_LAYOUT: NavLayout = [
  { target: { kind: "top", id: "brand" }, rect: { x: 0, y: 0, w: 120, h: 40 } },
  { target: { kind: "top", id: "palette" }, rect: { x: 130, y: 0, w: 140, h: 40 } },
  { target: { kind: "top", id: "layout" }, rect: { x: 800, y: 0, w: 60, h: 40 } },
  ...[1, 2, 3, 5, 6].map((number, i) => ({ target: { kind: "row" as const, number }, rect: { x: 0, y: 40 + i * 56, w: 288, h: 56 } })),
  { target: { kind: "pane", index: 0 }, rect: { x: 288, y: 40, w: 300, h: 300 } },
  { target: { kind: "pane", index: 1 }, rect: { x: 588, y: 40, w: 300, h: 300 } },
  { target: { kind: "pane", index: 2 }, rect: { x: 288, y: 340, w: 300, h: 300 } },
  { target: { kind: "pane", index: 3 }, rect: { x: 588, y: 340, w: 300, h: 300 } },
];

export const NAMES: Record<number, string> = { 1: "billing", 2: "infra", 3: "api-server", 5: "docs-site", 6: "scratch", 7: "PowerShell" };

/** Sessions 1, 2, 3 and 6 are on screen in slots 0 to 3; 5 is only in the sidebar. */
export const GRID_CONTEXT: NavContext = { panes: [1, 2, 3, 6], layout: GRID_LAYOUT, names: NAMES };
