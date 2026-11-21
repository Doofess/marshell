import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { PaneTab } from "../../generated/PaneTab";
import { currentEntry, isBrowserTab } from "../pane/paneState";
import { BLANK } from "./address";

export type MenuTarget = { kind: "none" } | { kind: "blank" } | { kind: "file" } | { kind: "page"; url: string };

const COPY = "copy";
const OUTSIDE = "outside";
const HTML = "html";
const CLEAR = "clear";

/** What the Browser menu acts on: the Browser panel's selected tab. */
export function menuTarget(tab: PaneTab | undefined): MenuTarget {
  if (tab === undefined) return { kind: "none" };
  if (!isBrowserTab(tab)) return { kind: "file" };
  const url = currentEntry(tab).path;
  return url === "" || url === BLANK ? { kind: "blank" } : { kind: "page", url };
}

/** The Browser menu's rows; a row that can't run says why. */
export function browserMenuItems(t: MenuTarget, pagesAvailable: boolean): MenuItem[] {
  const noAddress = t.kind === "page" ? undefined : t.kind === "file" ? "An HTML file has no address" : "No page is open";
  const items: MenuItem[] = [
    { id: COPY, label: "Copy link", disabled: noAddress },
    { id: OUTSIDE, label: "Open in the system browser", disabled: noAddress },
    { separator: true },
    { id: HTML, label: "Open HTML file…" },
    { separator: true },
    { id: CLEAR, label: "Clear browsing data…", disabled: pagesAvailable ? undefined : "Not available on this system yet" },
  ];
  return items;
}

export type BrowserMenuChoice = "copy" | "outside" | "html" | "clear" | null;

/** What a chosen row stands for; null for an id the menu never had. */
export function browserMenuChoice(id: string): BrowserMenuChoice {
  return id === COPY || id === OUTSIDE || id === HTML || id === CLEAR ? id : null;
}
