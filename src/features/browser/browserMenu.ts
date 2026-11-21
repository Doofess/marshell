import type { MenuItem } from "../../components/ContextMenu/ContextMenu";
import type { BrowserSettings } from "../../generated/BrowserSettings";
import type { CookieRule } from "../../generated/CookieRule";
import type { PaneTab } from "../../generated/PaneTab";
import { currentEntry, isBrowserTab } from "../pane/paneState";
import { BLANK } from "./address";
import { cookieChoice, cookieRows, SETTINGS_UNREAD } from "./cookies";
import { viewportChoice, viewportRows, type ViewportId } from "./viewport";

export type MenuTarget = { kind: "none" } | { kind: "blank" } | { kind: "file" } | { kind: "page"; url: string };

const COPY = "copy";
const SHOT = "shot";
const SHOT_FULL = "shot-full";
const OUTSIDE = "outside";
const HTML = "html";
const LINKS = "links";
const AUTO = "auto";
const VIEWPORT = "viewport";
const COOKIES = "cookies";
const CLEAR = "clear";
const NOT_YET = "Not available on this system yet";

/** What the Browser menu acts on: the Browser panel's selected tab. */
export function menuTarget(tab: PaneTab | undefined): MenuTarget {
  if (tab === undefined) return { kind: "none" };
  if (!isBrowserTab(tab)) return { kind: "file" };
  const url = currentEntry(tab).path;
  return url === "" || url === BLANK ? { kind: "blank" } : { kind: "page", url };
}

export type BrowserMenuState = {
  pages: boolean;
  /** Null until a read works: nothing is shown as chosen then. */
  settings: BrowserSettings | null;
  /** The read failed (`useBrowserSettings().failed`): the only case that says the settings couldn't be read. */
  failed: boolean;
  /** The tab's. */
  viewport: ViewportId | null;
  /** `shotReason(...)`: why no picture can be taken now. */
  shot: string | null;
  /** `autoOpenNote(...)` for the session on screen. */
  autoNote: string | null;
};

const off = (reason: string | null | undefined): { disabled?: string } => (reason ? { disabled: reason } : {});

/** The Browser menu's rows; a row that can't run says why. */
export function browserMenuItems(t: MenuTarget, s: BrowserMenuState): MenuItem[] {
  const noAddress = t.kind === "page" ? undefined : t.kind === "file" ? "An HTML file has no address" : "No page is open";
  const needPages = s.pages ? undefined : NOT_YET;
  const needSettings = s.settings === null && s.failed ? SETTINGS_UNREAD : undefined;
  return [
    { id: COPY, label: "Copy link", ...off(noAddress) },
    { id: SHOT, label: "Save screenshot", ...off(s.shot) },
    { id: SHOT_FULL, label: "Save full-page screenshot", ...off(s.shot) },
    { id: OUTSIDE, label: "Open in the system browser", ...off(noAddress) },
    { separator: true },
    { id: HTML, label: "Open HTML file…" },
    { separator: true },
    { id: LINKS, label: "Open terminal links here", checked: s.settings?.terminal_links === true, ...off(needPages ?? needSettings) },
    { id: AUTO, label: "Open HTML files agents write", checked: s.settings?.open_agent_html === true, ...(s.autoNote === null ? {} : { note: s.autoNote }), ...off(needSettings) },
    { id: VIEWPORT, label: "Viewport", rows: viewportRows(s.viewport), ...off(t.kind === "none" ? "No tab is open" : undefined) },
    { id: COOKIES, label: "Cookies", rows: cookieRows(s.settings?.cookies ?? null, s.pages, s.failed), ...off(needPages) },
    { separator: true },
    { id: CLEAR, label: "Clear browsing data…", ...off(needPages) },
  ];
}

export type BrowserMenuChoice =
  | { kind: "copy" | "outside" | "html" | "clear" | "links" | "auto" }
  | { kind: "shot"; full: boolean }
  | { kind: "viewport"; viewport: ViewportId | null }
  | { kind: "cookies"; rule: CookieRule }
  | null;

/** What a chosen row stands for; null for an id the menu never had, and for a row that only opens a submenu. */
export function browserMenuChoice(id: string): BrowserMenuChoice {
  switch (id) {
    case COPY:
    case OUTSIDE:
    case HTML:
    case CLEAR:
    case LINKS:
    case AUTO:
      return { kind: id };
    case SHOT:
      return { kind: "shot", full: false };
    case SHOT_FULL:
      return { kind: "shot", full: true };
  }
  const viewport = viewportChoice(id);
  if (viewport !== undefined) return { kind: "viewport", viewport };
  const rule = cookieChoice(id);
  return rule === undefined ? null : { kind: "cookies", rule };
}

export const toggledText = (label: string, on: boolean): string => `${label}: ${on ? "on" : "off"}`;
