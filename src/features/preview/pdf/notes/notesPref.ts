import { HIGHLIGHT_COLORS } from "./editor";

/** The colour the person last picked, kept while Marshell runs and shared by every PDF tab (as `sidebarPref.ts` keeps the sidebar). A restart starts on Yellow. */
let picked: string = HIGHLIGHT_COLORS[0]!.value;

export function colorPref(): string {
  return picked;
}

export function setColorPref(hex: string): void {
  if (HIGHLIGHT_COLORS.some((c) => c.value === hex)) picked = hex;
}
