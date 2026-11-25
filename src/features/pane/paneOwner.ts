/** The pane's key for the shell panel's tab set (`SHELL_OWNER` in crates/core/src/layout.rs). */
export const SHELL_OWNER = "shell";
export type OwnerPref = "shell" | "session";
/** Whose tabs the pane shows: `key` keys the pane state, `tabId` is the core's tab (null while the panel's shell isn't there). */
export type PaneOwner = { key: string; tabId: string | null; shell: boolean };
export const SHELL_PANEL_AREA = ".shell-panel";
export const PANE_AREA = ".window__pane, .pane__divider";
export const SESSION_AREA = ".window__main, .window__side";

/** What focus on `el` says about the owner: the shell panel, a session's terminal or the sidebar, or nothing (null keeps the last). */
export function focusOwnerOf(el: Pick<Element, "closest"> | null): OwnerPref | null {
  if (!el) return null;
  if (el.closest(PANE_AREA)) return null;
  // The panel sits inside `.window__side`, so it is checked before the session area.
  if (el.closest(SHELL_PANEL_AREA)) return "shell";
  if (el.closest(SESSION_AREA)) return "session";
  return null;
}

/** No session: the shell. Else the shell while it was focused last and the panel is on screen, else the focused session. */
export function paneOwner(i: { pref: OwnerPref; focusedId: string | undefined; shellShown: boolean; panelTabId: string | undefined }): PaneOwner {
  if (i.focusedId === undefined || (i.pref === "shell" && i.shellShown)) return { key: SHELL_OWNER, tabId: i.panelTabId ?? null, shell: true };
  return { key: i.focusedId, tabId: i.focusedId, shell: false };
}

/** The pane key a core tab id is under: the shell's while it is the panel's shell, else the id itself. */
export function ownerKeyOf(tabId: string, panelTabId: string | undefined): string {
  return panelTabId !== undefined && tabId === panelTabId ? SHELL_OWNER : tabId;
}

/** A tab's key for pages and the tab maps: the panel shell's id stands in for the shell's key; with no shell, `shell/<tab>`. */
export function ownerPageKey(key: string, paneTab: string, panelTabId: string | undefined): string {
  return `${key === SHELL_OWNER && panelTabId !== undefined ? panelTabId : key}/${paneTab}`;
}

/** `set` with `from` replaced by `to` (for the sessions told of the tab limit); the same set when `from` isn't in it. */
export function renamed(set: ReadonlySet<string>, from: string, to: string): ReadonlySet<string> {
  if (!set.has(from)) return set;
  return new Set([...set].map((k) => (k === from ? to : k)));
}

/** Where the shell's tabs deliver: the panel, through the clipboard (it has no prompt line). The name starts the lines that use it. */
export const SHELL_DELIVERY: { name: string; via: "clipboard" } = { name: "The shell panel", via: "clipboard" };
