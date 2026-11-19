/**
 * What the person last chose for the sidebar, kept while Marshell runs and shared by every PDF tab (slice 6q-5, decision 10).
 * It is module state: a restart starts closed. It records choices only, so a sidebar that closed after a pick, by Esc or by a
 * click outside it, and a view shown for want of an outline, leave it as it was.
 */
export type SidebarView = "outline" | "pages" | "notes";
export type SidebarPref = { open: boolean; view: SidebarView | null };

let pref: SidebarPref = { open: false, view: null };

export function sidebarPref(): SidebarPref {
  return pref;
}

export function setSidebarPref(p: Partial<SidebarPref>): void {
  pref = { ...pref, ...p };
}

/** A tab opens its sidebar only when the person left it open, and only where it wouldn't lie over the pages. */
export function startOpen(p: SidebarPref, overlay: boolean): boolean {
  return p.open && !overlay;
}

/**
 * Whether the keyboard has left a sidebar lying over the pages, read a moment after it went out: the page still has the focus,
 * on something outside the sidebar or on nothing. A window that lost the focus (Alt+Tab) keeps it in the sidebar, which stays open.
 */
export function keyboardLeft(pageFocused: boolean, active: unknown, inside: (t: unknown) => boolean): boolean {
  return pageFocused && !inside(active);
}

/** The view a tab opens on: the one last chosen, Pages for a PDF with no outline, and else Outline. */
export function startView(p: SidebarPref, hasOutline: boolean): SidebarView {
  if (!hasOutline) return "pages";
  return p.view ?? "outline";
}
